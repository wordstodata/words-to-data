//! A filter that matches nothing in held data says so, and does not say that
//! the dataset holds nothing (#425).
//!
//! `section-agreement --bill 119-hr-2215` said "This dataset holds no amendment
//! link to check." on a dataset that holds 3,224. `redesignation-report
//! --bill-id 119-hr-42` said "This dataset holds no bill that states a
//! renumbering." where four bills state one. Each filter was held, and each
//! matched nothing: that is an answer, so the run exits 0, and its words name
//! the filter.
//!
//! Every input is committed. The links are the real output of one run of the
//! evidence method over the corpus, all for `119-hr-1`. The bills are the
//! committed laws `119-hr-1` (Pub. L. 119-21) and `119-hr-42` (Pub. L. 119-22),
//! as the Congress client hands them over.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::link::Link;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_parser::parse_bill_amendments;

const CACHE: &str = "tests/test_data/congress_client_cache/bill/119";
const EVIDENCE_LINKS: &str = "tests/test_data/processed/evidence_links.json";

/// The bill every link of the fixture names.
const LINKED: &str = "119-hr-1";
/// A held bill that no link names, and that states no renumbering.
const UNLINKED: &str = "119-hr-42";
/// A held bill whose amendments are held and whose document is not, as in a
/// dataset that holds the bill and not the law it became.
const NO_DOCUMENT: &str = "119-s-1071";

/// A committed law with its metadata, as the Congress client hands it over.
fn download(dir: &str, bill_id: &str) -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{CACHE}/{dir}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: bill_id.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// Three held bills, the document of two of them, and the links of one, in a file built once.
fn two_bills() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut dataset: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&download("hr/1", LINKED))
            .expect("the law should load");
        dataset
            .load_bill_download(&download("hr/42", UNLINKED))
            .expect("the law should load");
        let law = format!("{CACHE}/s/1071/public_law.xml");
        dataset
            .add_bill(parse_bill_amendments(NO_DOCUMENT, &law).expect("the law should parse"))
            .expect("the bill should add");
        let json = std::fs::read_to_string(EVIDENCE_LINKS).expect("the fixture should be readable");
        let links: Vec<Link> =
            serde_json::from_str(&json).expect("the fixture should parse as links");
        for link in links {
            dataset.add_link(link).expect("the link should add");
        }
        let path = format!("{}/matched_nothing.json", env!("CARGO_TARGET_TMPDIR"));
        dataset
            .save(&path, Format::Compact)
            .expect("the fixture should save");
        path
    })
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn should_say_no_link_matches_the_bill_when_section_agreement_is_given_a_held_bill_with_no_link() {
    let output = run(&["section-agreement", two_bills(), "--bill", UNLINKED]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = stdout(&output);
    assert!(
        text.contains(&format!("No amendment link matches --bill {UNLINKED}.")),
        "{text}"
    );
    assert!(!text.contains("holds no amendment link"), "{text}");
}

/// No expression holds this path: section 9999 of a chapter of title 22.
const PATH_NOT_HELD: &str = "uscode/title_22/chapter_103/section_9999";

/// The not-held rule of #392: exit 1, the notice on stderr, nothing on stdout.
fn assert_refused(output: &Output, notice: &str) {
    assert_eq!(output.status.code(), Some(1), "{}", stdout(output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(notice), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", stdout(output));
}

#[test]
fn should_refuse_and_name_the_path_when_section_agreement_is_given_a_path_no_link_or_expression_holds()
 {
    for json in [false, true] {
        let mut args = vec!["section-agreement", two_bills(), "--path", PATH_NOT_HELD];
        if json {
            args.push("--json");
        }
        assert_refused(
            &run(&args),
            &format!("No expression in the dataset holds {PATH_NOT_HELD} or a path beneath it."),
        );
    }
}

#[test]
fn should_say_the_bill_states_no_renumbering_when_redesignation_report_is_given_a_held_bill_with_none()
 {
    let output = run(&["redesignation-report", two_bills(), "--bill-id", UNLINKED]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = stdout(&output);
    assert!(
        text.contains(&format!("Bill {UNLINKED} states no renumbering.")),
        "{text}"
    );
    assert!(!text.contains("This dataset holds no bill"), "{text}");
}

/// "A dataset built before #196 has none to read" was said of every held bill
/// with no document, also in a dataset built after #196, where the usual cause
/// is a bill that did not become law (#425).
#[test]
fn should_give_each_reason_when_amendment_addresses_is_given_a_held_bill_with_no_document() {
    let output = run(&["amendment-addresses", two_bills(), "--bill", NO_DOCUMENT]);

    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!(
            "This dataset holds bill {NO_DOCUMENT}, and no public-law document that states its amendments"
        )),
        "{stderr}"
    );
    assert!(stderr.contains("did not become law"), "{stderr}");
}
