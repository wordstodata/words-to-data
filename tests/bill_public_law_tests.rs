//! `bills` and `show-bill` name the public law each bill became, and their
//! counts are true (#297).
//!
//! No command said that `119-hr-1` became Pub. L. 119-21: an agent found it only
//! in the amendment paths of `show-bill --json`. And the WITH CHANGES column was
//! 0 for every bill, with "(0 change(s))" on every amendment, also where
//! `link-by-evidence` had linked hundreds of them. It read a field that nothing
//! fills since #252.
//!
//! Every case reads committed public laws: `119-hr-1` (Pub. L. 119-21) and
//! `119-s-1071` (Pub. L. 119-60), each stored as its document, and `119-hr-998`
//! with its amendments and no document.

use std::process::{Command, Output};
use std::sync::OnceLock;

use std::collections::BTreeMap;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::inspect;
use words_to_data::link::{LinkKind, amendment_reference_parts};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const CACHE: &str = "tests/test_data/congress_client_cache/bill/119";

/// The bill as its amendments, and its law as a document when `with_law`.
fn add(dataset: &mut Dataset<InMemoryStorage>, bill_id: &str, dir: &str, with_law: bool) {
    let path = format!("{CACHE}/{dir}/public_law.xml");
    let bill = parse_bill_amendments(bill_id, &path).expect("the bill should parse");
    dataset.add_bill(bill).expect("the bill should add");
    if with_law {
        let xml = std::fs::read_to_string(&path).expect("the law should read");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, bill_id).expect("the law should read");
        dataset.add_expression(law).expect("the law should add");
    }
}

/// Two bills with their laws, and one bill without its law.
fn laws() -> &'static Dataset<InMemoryStorage> {
    static FIXTURE: OnceLock<Dataset<InMemoryStorage>> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        add(&mut dataset, "119-hr-1", "hr/1", true);
        add(&mut dataset, "119-s-1071", "s/1071", true);
        add(&mut dataset, "119-hr-998", "hr/998", false);
        dataset
    })
}

#[test]
fn should_list_the_public_law_of_each_bill_when_the_dataset_holds_its_document() {
    let bills = inspect::bills(laws()).expect("the bills should list");

    let law_of = |id: &str| {
        bills
            .iter()
            .find(|bill| bill.bill_id == id)
            .unwrap_or_else(|| panic!("{id} should be listed"))
            .public_law
            .clone()
    };
    assert_eq!(law_of("119-hr-1").as_deref(), Some("119-21"));
    assert_eq!(law_of("119-s-1071").as_deref(), Some("119-60"));
    assert_eq!(
        law_of("119-hr-998"),
        None,
        "the dataset holds no document for this bill"
    );
}

#[test]
fn should_name_the_public_law_in_a_bill_summary_when_the_dataset_holds_its_document() {
    let enacted = inspect::show_bill(laws(), "119-s-1071")
        .expect("the bill should read")
        .expect("the bill is held");
    let without_law = inspect::show_bill(laws(), "119-hr-998")
        .expect("the bill should read")
        .expect("the bill is held");

    assert_eq!(enacted.public_law.as_deref(), Some("119-60"));
    assert_eq!(without_law.public_law, None);
}

/// The same dataset, saved as a database for the binary to open.
fn laws_on_disk() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/bill_public_law.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        laws()
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// One run of the binary, the way an agent or a shell runs it.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_show_the_bill_a_public_law_was_when_show_bill_is_given_the_law() {
    let dataset = laws_on_disk();

    for law in [
        "Pub. L. 119-21",
        "119-21",
        "PL 119-21",
        "Public Law 119\u{2013}21",
    ] {
        let output = run(&["show-bill", dataset, law, "--json"]);
        assert!(
            output.status.success(),
            "show-bill should accept {law:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let shown: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("show-bill --json should emit json");
        assert_eq!(shown["bill_id"], "119-hr-1", "for {law:?}");
        assert_eq!(shown["public_law"], "119-21", "for {law:?}");
    }
}

#[test]
fn should_refuse_by_name_when_show_bill_is_given_a_law_the_dataset_does_not_hold() {
    let output = run(&["show-bill", laws_on_disk(), "Pub. L. 119-22", "--json"]);

    assert_eq!(output.status.code(), Some(1), "a law not held is refused");
    assert!(output.stdout.is_empty(), "and no half answer is printed");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains("Pub. L. 119-22") && said.contains("bills"),
        "the refusal names the law and the command that lists the held ones, got: {said}"
    );
}

/// The committed Code release points, at which `link-by-evidence` links
/// amendments of `119-hr-1` in titles 7 and 26.
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// `119-hr-1` and its law with titles 7 and 26 at each committed release point,
/// linked by `link-by-evidence`, and then one of its links refuted by `settle`.
///
/// Returns the database and the id of the refuted link.
fn linked() -> &'static (String, String) {
    static FIXTURE: OnceLock<(String, String)> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!(
            "{}/bill_public_law_linked.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata::default());
        add(&mut dataset, "119-hr-1", "hr/1", true);
        for date in RELEASE_POINTS {
            for title in ["usc07.xml", "usc26.xml"] {
                dataset
                    .add_uslm_xml(&format!("tests/test_data/usc/{date}/{title}"), date, None)
                    .expect("the title should parse");
            }
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");

        let linking = run(&["link-by-evidence", &path]);
        assert!(
            linking.status.success(),
            "link-by-evidence should exit zero, stderr: {}",
            String::from_utf8_lossy(&linking.stderr)
        );

        let refuted = Dataset::open_sqlite(&path)
            .expect("the linked dataset should open")
            .links_by_kind(LinkKind::AMENDED_BY)
            .expect("the links should read")
            .first()
            .expect("link-by-evidence links amendments of the committed law")
            .id();
        let settling = run(&[
            "settle",
            &path,
            "--link",
            &refuted,
            "--verdict",
            "refuted",
            "--reviewer",
            "human:test",
            "--reason",
            "a refuted link does not stand",
        ]);
        assert!(
            settling.status.success(),
            "settle should exit zero, stderr: {}",
            String::from_utf8_lossy(&settling.stderr)
        );
        (path, refuted)
    })
}

/// How many `amended_by` links name each amendment of `119-hr-1`, read from
/// the stored links, leaving out the one the fixture refuted.
fn standing_links_by_amendment() -> BTreeMap<String, u64> {
    let (path, refuted) = linked();
    let mut counts = BTreeMap::new();
    for link in Dataset::open_sqlite(path)
        .expect("the linked dataset should open")
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("the links should read")
    {
        if &link.id() == refuted {
            continue;
        }
        let reference = link.object.name();
        if let Some(("119-hr-1", amendment)) = amendment_reference_parts(&reference) {
            *counts.entry(amendment.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

#[test]
fn should_count_the_standing_links_that_name_each_amendment_when_show_bill_lists_them() {
    let (path, _) = linked();
    let expected = standing_links_by_amendment();
    assert!(!expected.is_empty(), "the fixture holds standing links");

    let output = run(&["show-bill", path, "119-hr-1", "--json"]);
    assert!(
        output.status.success(),
        "show-bill should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let shown: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("show-bill --json should emit json");
    for amendment in shown["amendments"]
        .as_array()
        .expect("a list of amendments")
    {
        let id = amendment["id"].as_str().expect("an amendment id");
        assert_eq!(
            amendment["link_count"].as_u64(),
            Some(expected.get(id).copied().unwrap_or(0)),
            "amendment {id}"
        );
    }
}

#[test]
fn should_count_the_amendments_a_standing_link_names_when_bills_lists_a_bill() {
    let (path, _) = linked();

    let output = run(&["bills", path, "--json"]);
    assert!(
        output.status.success(),
        "bills should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("bills --json should emit json");
    assert_eq!(listed[0]["bill_id"], "119-hr-1");
    assert_eq!(
        listed[0]["amendments_linked"].as_u64(),
        Some(standing_links_by_amendment().len() as u64)
    );
}

#[test]
fn should_print_each_bill_with_its_public_law_on_one_line_when_bills_is_read_as_text() {
    let output = run(&["bills", laws_on_disk()]);

    assert!(
        output.status.success(),
        "bills should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let said = String::from_utf8_lossy(&output.stdout);
    let line_of = |id: &str| {
        said.lines()
            .find(|line| line.starts_with(id))
            .unwrap_or_else(|| panic!("{id} should have a line, got:\n{said}"))
            .to_string()
    };
    assert!(said.contains("PUBLIC LAW"), "got:\n{said}");
    assert!(
        line_of("119-hr-1 ").contains("Pub. L. 119-21"),
        "got:\n{said}"
    );
    assert!(
        line_of("119-s-1071").contains("Pub. L. 119-60"),
        "got:\n{said}"
    );
    assert!(
        line_of("119-hr-998").contains("not enacted / not held"),
        "got:\n{said}"
    );
    assert!(!said.contains("WITH CHANGES"), "got:\n{said}");
}

#[test]
fn should_give_the_public_law_and_no_change_count_when_show_bill_is_read_as_text() {
    let (path, _) = linked();

    let output = run(&["show-bill", path, "119-hr-1"]);

    assert!(
        output.status.success(),
        "show-bill should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let said = String::from_utf8_lossy(&output.stdout);
    let header: Vec<&str> = said.lines().take(3).collect();
    assert!(
        header.iter().any(|line| line.contains("Pub. L. 119-21")),
        "the header names the law, got:\n{}",
        header.join("\n")
    );
    assert!(
        !said.contains("change(s)"),
        "no amendment shows a count of changes nothing extracts"
    );
    let linked_lines = said
        .lines()
        .filter(|line| line.starts_with("- ") && !line.contains("(0 standing link(s))"))
        .count();
    assert_eq!(
        linked_lines,
        standing_links_by_amendment().len(),
        "each amendment a standing link names shows its count"
    );
}
