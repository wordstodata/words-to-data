//! A Statutes at Large cite of a Revenue Code just before a 26 U.S.C. cite
//! names the edition of the Code that the court read (#482).
//!
//! *Commissioner v. Glenshaw Glass Co.* (1955) writes in its first footnote
//! "53 Stat. 9, 53 Stat. 574, 26 U. S. C. § 22 (a)". Volume 53 of the
//! Statutes at Large holds the Internal Revenue Code of 1939, and § 22(a) of
//! that Code is the gross-income rule. In the printings held, § 22(a) of title
//! 26 is the credit for the elderly. So the citation records the edition of
//! 1939, the same as a citation with "(1976 ed.)" after it (#449).
//!
//! Every record here is real: title 26 from the committed corpus, and the
//! committed CourtListener records in `tests/test_data/courtlistener`.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::citation::usc;
use words_to_data::courtlistener::OpinionRecord;
use words_to_data::dataset::{Dataset, DatasetMetadata};

const CACHE: &str = "tests/test_data/courtlistener";

/// The opinion of the Court in *Commissioner v. Glenshaw Glass Co.* (1955).
const GLENSHAW_GLASS: u64 = 105281;

/// The opinion of the Court in *Moore v. United States* (2024). It cites
/// "76 Stat. 1006, 26 U. S. C. § 951 et seq.", the Revenue Act of 1962, which
/// is not a Revenue Code, and "See 26 U. S. C. § 11." with no Stat. cite.
const MOORE: u64 = 11066657;

/// The text the dataset stores for one opinion record.
fn stored_text(id: u64) -> String {
    let json = std::fs::read_to_string(format!("{CACHE}/opinion_{id}.json"))
        .unwrap_or_else(|error| panic!("opinion {id} should be committed: {error}"));
    let record = OpinionRecord::from_json(&json).expect("the opinion record should read");
    let (text, _, _) = record.text().expect("the record carries text");
    text
}

/// The one citation in `found` of `section` of title 26.
fn the_citation<'a>(found: &'a [usc::UscCitation], section: &str) -> &'a usc::UscCitation {
    let [cited] = found
        .iter()
        .filter(|citation| citation.title == "26" && citation.sections == [section])
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| panic!("the opinion cites § {section} once, got {found:?}"));
    cited
}

#[test]
fn should_record_the_1939_edition_when_53_stat_comes_just_before_a_26_usc_cite() {
    let found = usc::find(&stored_text(GLENSHAW_GLASS));

    let cited = the_citation(&found, "22(a)");
    let edition = cited
        .edition
        .as_ref()
        .expect("the Stat. cite names the Code of 1939");
    assert_eq!(edition.year, "1939");
    assert_eq!(edition.written, "53 Stat. 574");
}

#[test]
fn should_record_no_edition_when_a_26_usc_cite_has_no_stat_cite_before_it() {
    let found = usc::find(&stored_text(MOORE));

    let cited = the_citation(&found, "11");
    assert_eq!(cited.edition, None);
}

#[test]
fn should_record_no_edition_when_the_stat_cite_before_it_is_not_a_revenue_code() {
    let text = stored_text(MOORE);
    let found = usc::find(&text);

    let [cited] = found
        .iter()
        .filter(|citation| text[..citation.start].ends_with("76 Stat. 1006, "))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| panic!("Moore cites one section after 76 Stat. 1006, got {found:?}"));
    assert_eq!(cited.sections, ["951"]);
    assert_eq!(cited.edition, None, "76 Stat. is the Revenue Act of 1962");
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// A printing of title 26 and *Glenshaw Glass*, added by `add-opinions`
/// offline from the committed CourtListener records.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/stat_edition.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and Glenshaw Glass".to_string(),
            ..Default::default()
        });
        let file = "tests/test_data/usc/2025-07-18/usc26.xml";
        dataset
            .add_uslm_xml(file, "2025-07-18", None)
            .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
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
            &GLENSHAW_GLASS.to_string(),
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the opinion, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

#[test]
fn should_name_the_1939_edition_when_cases_citing_reads_a_stat_cite_of_the_code() {
    let output = run(&[
        "cases-citing",
        dataset_path(),
        "--cites",
        "26 U.S.C. \u{a7} 22(a)",
    ]);
    assert!(
        output.status.success(),
        "cases-citing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains(
            "no window is given: the opinion cites the Code of 1939, written \"53 Stat. 574\" \
             before the citation, and this dataset holds no printing of title 26 from that year."
        ),
        "the edition is named, got:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "the opinion cites the Code of 1939, written \"53 Stat. 574\" before the citation. \
             This dataset holds no printing of title 26 from 1939, so that law is out of scope."
        ),
        "the line on the place names the edition, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("unchanged") && !stdout.contains("CHANGED"),
        "no window is given, got:\n{stdout}"
    );
}
