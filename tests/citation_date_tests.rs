//! A citation carries the date the court decided the case, the year the
//! opinion states it applies, and the printing its object was resolved in
//! (#317, `docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decision 7).
//!
//! The fixture is real: two printings of title 26 (2025-07-18 and 2025-07-30),
//! and committed CourtListener records added by `add-opinions` offline:
//! *Shami* (2014-01-23), decided before every printing held; *Mayo Clinic*
//! (2025-07-25), decided between the two; and *Khalaf* (2020-02-05), which
//! states that its references are to the Code "in effect for 2013".

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::link::{Link, LinkKind};
use words_to_data::storage::LinkReader;

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";
const SHAMI: &str = "judicial/case_2651100/opinion_2651100";
const MAYO: &str = "judicial/case_10641173/opinion_11107760";
const KHALAF: &str = "judicial/case_10140651/opinion_10607252";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/citation_date.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and three opinions".to_string(),
            description: "The date a citation carries".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        for date in ["2025-07-18", "2025-07-30"] {
            let file = format!("tests/test_data/usc/{date}/usc26.xml");
            dataset
                .add_uslm_xml(&file, date, None)
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
            "tests/test_data",
            "--opinions",
            "2651100,11107760,10607252",
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the three, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

/// Every citation link from a passage of `writing`.
fn links_from(writing: &str) -> Vec<Link> {
    Dataset::open_sqlite(dataset_path())
        .expect("the fixture should open")
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
        .into_iter()
        .filter(|link| {
            link.subject
                .path()
                .is_some_and(|subject| subject.starts_with(&format!("{writing}/")))
        })
        .collect()
}

fn payload(link: &Link) -> &serde_json::Value {
    &link
        .payload
        .as_ref()
        .expect("a citation has a payload")
        .value
}

#[test]
fn should_carry_the_decision_date_and_the_earliest_printing_when_the_court_decided_before_every_printing_held()
 {
    let links = links_from(SHAMI);
    let [cites_174] = links
        .iter()
        .filter(|link| link.object.path() == Some(SECTION_174))
        .collect::<Vec<_>>()
        .try_into()
        .expect("Shami cites § 174 in one passage");
    let value = payload(cites_174);
    assert_eq!(value["decided"], "2014-01-23");
    assert_eq!(
        value["resolved_in"], "2025-07-18",
        "no printing is held from 2014, so the earliest held is used"
    );
    assert_eq!(value["resolved_after_decision"], true);
    assert_eq!(value["occurrences"][0]["unresolved"], "(e)");
    for link in &links {
        assert_eq!(payload(link)["decided"], "2014-01-23", "{link:?}");
    }
}

#[test]
fn should_resolve_against_the_printing_on_or_before_the_decision_when_one_is_held() {
    // Mayo Clinic was decided on 2025-07-25: the 2025-07-18 printing is the law
    // in force that day, not the 2025-07-30 one.
    let links = links_from(MAYO);
    assert!(!links.is_empty(), "Mayo cites title 26");
    for link in &links {
        let value = payload(link);
        assert_eq!(value["decided"], "2025-07-25");
        assert_eq!(value["resolved_in"], "2025-07-18", "{link:?}");
        assert_eq!(value["resolved_after_decision"], false);
    }
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn should_say_which_printing_the_citation_was_read_in_and_that_the_decision_date_is_out_of_scope() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_174,
    ]));
    assert!(
        printed.contains(
            "decided 2014-01-23. The link was resolved in the printing of 2025-07-18, \
             the earliest held: this dataset holds no printing of the law as it stood \
             on 2014-01-23, so that law is out of scope."
        ),
        "got:\n{printed}"
    );
    assert!(
        printed.contains("2014-01-23 → 2025-07-18: OUT OF SCOPE"),
        "got:\n{printed}"
    );
}

#[test]
fn should_name_the_stated_tax_year_and_say_it_is_out_of_scope_when_no_printing_is_from_it() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_174,
    ]));
    let khalaf = printed
        .split("\n\n")
        .find(|block| block.contains(KHALAF))
        .unwrap_or_else(|| panic!("Khalaf cites § 174, got:\n{printed}"));
    assert!(
        khalaf.contains(
            "the opinion states it applies title 26 as in effect for 2013. This dataset \
             holds no printing of title 26 from 2013, so that law is out of scope."
        ),
        "got:\n{khalaf}"
    );
}
