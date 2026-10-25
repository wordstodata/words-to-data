//! A citation runs from the paragraph that holds it to the subdivision the
//! court wrote, and every place the court wrote it is in the payload (#316,
//! `docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 5 and 6).
//!
//! The fixture is real: two printings of title 26 from the committed corpus,
//! and the ten committed CourtListener records, added by `add-opinions`
//! offline. So the links are the links the command writes.

use std::collections::BTreeMap;
use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::citation::default_code;
use words_to_data::citation::resolve::{Resolution, SectionPaths, resolve};
use words_to_data::citation::usc;
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::link::{Link, LinkKind};
use words_to_data::storage::{LinkReader, SqliteStorage};

const TITLE_26_EARLIER: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_LATER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";

/// The ten opinions, as the command takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

/// The nine cases of the ten opinions, by cluster id.
const CASES: [u64; 9] = [
    2812209, 109019, 122262, 2651100, 6248, 7029371, 8998794, 1527901, 406879,
];

/// Thomas's dissent in *Boeing Co. v. United States* (2003).
const BOEING_DISSENT: &str = "judicial/case_122262/opinion_9434365";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Two printings of title 26 and the ten opinions, built once for the file.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/citation_subdivision.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and ten opinions".to_string(),
            description: "Citations from the paragraph to the subdivision".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        for (file, date) in [
            (TITLE_26_EARLIER, "2025-07-18"),
            (TITLE_26_LATER, "2025-07-30"),
        ] {
            dataset
                .add_uslm_xml(file, date, None)
                .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
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
            TEN,
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the ten, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

fn dataset() -> Dataset<SqliteStorage> {
    Dataset::open_sqlite(dataset_path()).expect("the fixture should open")
}

fn citation_links() -> Vec<Link> {
    dataset()
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
}

/// The words of the node at `path`, in the case that holds it.
fn words_at(path: &str) -> String {
    let found = dataset().find_nodes(path).expect("the store should answer");
    let (_, node) = found
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the dataset should hold {path}"));
    node.data
        .content
        .as_deref()
        .unwrap_or_else(|| panic!("{path} should hold words"))
        .to_string()
}

/// Every place in a link's payload, as (start, written form, words).
fn occurrences(link: &Link) -> Vec<(usize, String, String)> {
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
                place["start"].as_u64().expect("a place has a start") as usize,
                place["written"]
                    .as_str()
                    .expect("a place has the written form")
                    .to_string(),
                place["words"]
                    .as_str()
                    .expect("a place has the words")
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn should_run_from_the_paragraph_that_holds_it_to_the_subdivision_written_when_boeings_dissent_cites_174_a_1()
 {
    let object = format!("{SECTION_174}/subsection_a/paragraph_1");
    let links: Vec<Link> = citation_links()
        .into_iter()
        .filter(|link| {
            link.object.path() == Some(object.as_str())
                && link
                    .subject
                    .path()
                    .is_some_and(|subject| subject.starts_with(&format!("{BOEING_DISSENT}/")))
        })
        .collect();
    let [link] = links.as_slice() else {
        panic!("the dissent cites § 174(a)(1) from one paragraph: {links:#?}");
    };

    let paragraph = link.subject.path().expect("the subject is a path");
    assert!(
        paragraph.contains("/paragraph_"),
        "the subject is the paragraph, not the writing: {paragraph}"
    );
    let words = words_at(paragraph);
    let places = occurrences(link);
    assert!(!places.is_empty(), "the payload holds the place");
    for (start, written, matched) in places {
        assert_eq!(written, "174(a)(1)");
        assert!(
            words[start..].starts_with(&matched),
            "the start {start} is where the paragraph holds {matched:?}"
        );
    }
}

/// One committed record, as the API returned it.
fn read(name: String) -> String {
    std::fs::read_to_string(format!("{CACHE}/courtlistener/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

/// How many times each writing wrote each form of a section that resolves, read
/// from the whole text of the writing, as the reader read it before #316.
fn forms_written_in_whole_writings() -> BTreeMap<(String, String), usize> {
    let dataset = dataset();
    let title_26 = WorkId::new("uscode/title_26");
    let latest = ExpressionId::new(title_26, TITLE_26_LATEST_DATE);
    let printing = dataset
        .get_expression(&latest)
        .expect("the store should answer")
        .expect("the fixture holds the latest printing");
    let mut index = SectionPaths::new();
    index.add_work(&printing.root);
    let scope = dataset.scope().expect("the scope should derive");

    let mut forms = BTreeMap::new();
    for cluster_id in CASES {
        let cluster = ClusterRecord::from_json(&read(format!("cluster_{cluster_id}.json")))
            .expect("the cluster record should read");
        let opinions: Vec<OpinionRecord> = cluster
            .opinion_ids()
            .into_iter()
            .map(|id| {
                OpinionRecord::from_json(&read(format!("opinion_{id}.json")))
                    .expect("the opinion record should read")
            })
            .collect();
        let case = case_expression(&cluster, &opinions).expect("the case should build");
        let default = default_code::of_case(
            case.writings
                .iter()
                .map(|writing| (writing.path.as_str(), writing.text.as_str())),
        );
        for writing in &case.writings {
            let mut citations = usc::find(&writing.text);
            if let Some(default) = &default {
                let bare = default_code::find_bare(&writing.text, default, &citations);
                citations.extend(bare);
            }
            for citation in &citations {
                for section in resolve(citation, &scope, &index) {
                    // One place for each provision the form reaches: one for
                    // each path, and one for each member of a range of
                    // subdivisions (#366).
                    if let Resolution::Provision { .. } = &section.resolution {
                        *forms
                            .entry((writing.path.clone(), section.section.clone()))
                            .or_insert(0) += section.reached.len();
                    }
                }
            }
        }
    }
    forms
}

const TITLE_26_LATEST_DATE: &str = "2025-07-30";

/// The writing a passage belongs to: `judicial/case_<cluster>/opinion_<id>`.
fn writing_of(passage: &str) -> String {
    passage.split('/').take(3).collect::<Vec<_>>().join("/")
}

#[test]
fn should_keep_every_form_the_court_wrote_when_one_writing_cites_one_section_more_than_once() {
    // Before #316, a link was one writing and one section, so a second
    // citation of the section in the writing replaced the first: in the ten
    // opinions, 14 of 51 written forms were lost.
    let mut kept: BTreeMap<(String, String), usize> = BTreeMap::new();
    for link in citation_links() {
        let writing = writing_of(link.subject.path().expect("a held opinion is a path"));
        for (_, written, _) in occurrences(&link) {
            *kept.entry((writing.clone(), written)).or_insert(0) += 1;
        }
    }

    let written = forms_written_in_whole_writings();
    assert!(written.len() > 40, "the ten write many forms: {written:#?}");
    assert_eq!(kept, written, "every form written is a place in a link");
}

/// Title 26 as it read on 2025-07-30 and *Patricia A. Torres* (T.C. 2025),
/// whose bare references are read under the default code it states (#318).
fn torres_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/citation_subdivision_torres.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and Torres".to_string(),
            description: "One paragraph that cites one subdivision three times".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        dataset
            .add_uslm_xml(TITLE_26_LATER, TITLE_26_LATEST_DATE, None)
            .expect("title 26 should parse");
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
            "11136635",
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store Torres, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

#[test]
fn should_hold_every_place_in_one_link_when_one_paragraph_cites_one_subdivision_three_times() {
    // Judge Jenkins's dissent in Torres writes "section 199A(c)(1)" three
    // times in its tenth paragraph. One link names the paragraph and the
    // subdivision, so the three places are one link's payload.
    let paragraph = "judicial/case_10670048/opinion_11136635-2/paragraph_10";
    let object = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_199A/subsection_c/paragraph_1";
    let dataset = Dataset::open_sqlite(torres_path()).expect("the fixture should open");
    let links: Vec<Link> = dataset
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
        .into_iter()
        .filter(|link| link.subject.path() == Some(paragraph) && link.object.path() == Some(object))
        .collect();
    let [link] = links.as_slice() else {
        panic!("one link from the paragraph to § 199A(c)(1): {links:#?}");
    };

    let places = occurrences(link);
    assert_eq!(places.len(), 3, "{places:#?}");
    let (_, node) = dataset
        .find_nodes(paragraph)
        .expect("the store should answer")
        .into_iter()
        .next()
        .expect("the dataset holds the paragraph");
    let words = node
        .data
        .content
        .as_deref()
        .expect("a paragraph holds words");
    for (start, written, matched) in places {
        assert_eq!(written, "199A(c)(1)");
        assert!(
            words[start..].starts_with(&matched),
            "the paragraph holds {matched:?} at {start}"
        );
    }
}

/// Every place of every citation link from a writing to the provision at
/// `object`, as (start, written form, part that does not resolve).
fn unresolved_places(writing: &str, object: &str) -> Vec<(String, Option<String>)> {
    citation_links()
        .into_iter()
        .filter(|link| {
            link.object.path() == Some(object)
                && link
                    .subject
                    .path()
                    .is_some_and(|subject| subject.starts_with(&format!("{writing}/")))
        })
        .flat_map(|link| {
            let payload = link.payload.expect("a citation has a payload").value;
            payload["occurrences"]
                .as_array()
                .expect("the payload lists the places")
                .iter()
                .map(|place| {
                    (
                        place["written"].as_str().unwrap_or_default().to_string(),
                        place["unresolved"].as_str().map(str::to_string),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn should_end_at_the_deepest_part_that_resolves_and_name_the_rest_when_the_printing_does_not_hold_it()
 {
    // Shami (2014) cites § 174(e); § 174 runs from (a) to (d) in 2025. The
    // link ends at the section and names (e).
    assert_eq!(
        unresolved_places("judicial/case_2651100/opinion_2651100", SECTION_174),
        [("174(e)".to_string(), Some("(e)".to_string()))]
    );
    // United Stationers (1997) cites § 41(d)(l)(B)(i): OCR read (1) as (l).
    // The link ends at (d), the deepest part that resolves, and names the
    // rest as written. Nothing is repaired.
    let section_41 =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_41";
    assert_eq!(
        unresolved_places(
            "judicial/case_1527901/opinion_1527901",
            &format!("{section_41}/subsection_d")
        ),
        [("41(d)(l)(B)(i)".to_string(), Some("(l)(B)(i)".to_string()))]
    );
}

#[test]
fn should_list_the_same_writings_for_section_174_when_its_citations_end_below_the_section() {
    // Before #316, cases-citing on § 174 listed these nine writings, one link
    // each. Now a writing has a link for each passage and each subdivision it
    // wrote, and the reader still lists each writing once, with every provision
    // it cites in § 174.
    let printed = run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_174,
        "--json",
    ]);
    assert!(printed.status.success());
    let answers: serde_json::Value =
        serde_json::from_slice(&printed.stdout).expect("--json prints JSON");
    let cases = answers[0]["cases"].as_array().expect("cases is a list");
    let writings: Vec<&str> = cases
        .iter()
        .map(|case| {
            case["citing"]["held"]["path"]
                .as_str()
                .expect("a held writing")
        })
        .collect();
    let mut sorted = writings.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        writings.len(),
        "each writing once: {writings:?}"
    );
    assert_eq!(
        sorted,
        [
            "judicial/case_109019/opinion_109019",
            "judicial/case_122262/opinion_9434364",
            "judicial/case_122262/opinion_9434365",
            "judicial/case_1527901/opinion_1527901",
            "judicial/case_2651100/opinion_2651100",
            "judicial/case_406879/opinion_406879",
            "judicial/case_6248/opinion_6248",
            "judicial/case_7029371/opinion_6931314",
            "judicial/case_8998794/opinion_8991218",
        ]
    );
    let dissent = cases
        .iter()
        .find(|case| case["citing"]["held"]["path"] == BOEING_DISSENT)
        .expect("the dissent cites § 174");
    let cited: Vec<&str> = dissent["cites"]
        .as_array()
        .expect("cites is a list")
        .iter()
        .map(|cited| cited["path"].as_str().expect("a path"))
        .collect();
    assert_eq!(
        cited,
        [
            SECTION_174.to_string(),
            format!("{SECTION_174}/subsection_a/paragraph_1")
        ],
        "the dissent writes § 174 and § 174(a)(1)"
    );
}

#[test]
fn should_start_a_citation_at_the_passage_of_its_first_word_when_it_runs_on_into_the_next() {
    // In the Court's opinion in Torres, "I.R.C." ends one paragraph and
    // "§ 199A(b)(4)(B)" starts the next: the plain-text rule found a paragraph
    // break inside the citation. The citation is read across the break, and
    // starts at the paragraph that holds "I.R.C.".
    let dataset = Dataset::open_sqlite(torres_path()).expect("the fixture should open");
    let majority = "judicial/case_10670048/opinion_11136635-1/";
    let found: Vec<(String, usize)> = dataset
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
        .iter()
        .filter(|link| {
            link.subject
                .path()
                .is_some_and(|subject| subject.starts_with(majority))
        })
        .flat_map(|link| {
            let subject = link.subject.path().expect("a path").to_string();
            occurrences(link)
                .into_iter()
                .filter(|(_, _, words)| words == "I.R.C.\n\n§ 199A(b)(4)(B)")
                .map(move |(start, _, _)| (subject.clone(), start))
        })
        .collect();
    let [(paragraph, start)] = found.as_slice() else {
        panic!("one place holds the citation across the break: {found:?}");
    };
    let words = dataset
        .find_nodes(paragraph)
        .expect("the store should answer")
        .into_iter()
        .next()
        .and_then(|(_, node)| node.data.content.as_deref().map(str::to_string))
        .expect("the paragraph holds words");
    assert_eq!(
        &words[*start..],
        "I.R.C.",
        "the paragraph ends with its first word"
    );
}
