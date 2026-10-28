//! The counts `residue` prints say what they count (#469).
//!
//! Since #434, an amendment that acts on many sections has one row for each
//! item of its list. The text output printed the number of rows under the
//! label "amendment(s)", so an agent that added `bills` LINKED and the
//! `residue` headline got a sum that was not the number of amendments.
//!
//! The case is `119-s-1071` (Pub. L. 119-60) with title 10 at 2025-08-14 and
//! 2026-01-23 and title 33 at 2026-01-23 and 2026-07-12, all committed. Its
//! amendment 7049142d0b26 has two work rows (law section 3113(a)(1) and
//! 3113(a)(2)). Its amendment c1213b7b65b7, which amends the Oil Pollution Act
//! of 1990 in seven items, has a work row (7701(g)(5)) and six rows the
//! evidence method links and no link is written for. No link is written,
//! because the batch does not run here. Nothing is mocked.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const BILL: &str = "119-s-1071";
const LAW: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";

/// The law and the releases of titles 10 and 33, saved as a database for the
/// binary to open.
fn dataset() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill = parse_bill_amendments(BILL, LAW).expect("the bill should parse");
        dataset.add_bill(bill).expect("the bill should add");
        let xml = std::fs::read_to_string(LAW).expect("the law should read");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, BILL).expect("the law should read");
        dataset.add_expression(law).expect("the law should add");
        for (date, title) in [
            ("2025-08-14", "usc10.xml"),
            ("2026-01-23", "usc10.xml"),
            ("2026-01-23", "usc33.xml"),
            ("2026-07-12", "usc33.xml"),
        ] {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/{title}"), date, None)
                .expect("the title should parse");
        }
        let path = PathBuf::from(format!(
            "{}/residue_counts.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        ));
        let _ = std::fs::remove_file(&path);
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// What `words_to_data residue <dataset> --bill 119-s-1071 <args>` prints.
fn residue(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg("residue")
        .arg(dataset())
        .args(["--bill", BILL])
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "residue should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the output should be UTF-8")
}

/// The rows `--json` gives: each amendment id with the categories of its rows,
/// one entry for each row.
fn rows() -> &'static Vec<(String, String)> {
    static ROWS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let listing: serde_json::Value =
            serde_json::from_str(&residue(&["--json"])).expect("--json should emit json");
        listing["rows"]
            .as_array()
            .expect("a rows array")
            .iter()
            .map(|row| {
                (
                    row["amendment_id"].as_str().expect("an id").to_string(),
                    row["category"].as_str().expect("a category").to_string(),
                )
            })
            .collect()
    })
}

/// The categories of the rows of the amendment whose id starts with `short`.
fn categories_of(short: &str) -> Vec<&'static str> {
    rows()
        .iter()
        .filter(|(id, _)| id.starts_with(short))
        .map(|(_, category)| category.as_str())
        .collect()
}

#[test]
fn should_give_the_rows_and_the_amendments_in_the_headline_when_an_amendment_has_more_than_one_row()
{
    assert_eq!(categories_of("7049142d0b26"), ["work", "work"]);
    let amendments: BTreeSet<&str> = rows().iter().map(|(id, _)| id.as_str()).collect();

    let text = residue(&[]);

    let headline = text.lines().next().expect("a headline");
    assert_eq!(
        headline,
        format!(
            "{} row(s) for {} amendment(s) that no standing link names",
            rows().len(),
            amendments.len()
        )
    );
}

/// The lines of `text`, each with its runs of spaces made one space.
fn plain_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// How many rows of `category` there are, and for how many amendments.
fn rows_and_amendments(category: &str) -> (usize, usize) {
    let of_category: Vec<&str> = rows()
        .iter()
        .filter(|(_, c)| c == category)
        .map(|(id, _)| id.as_str())
        .collect();
    let amendments: BTreeSet<&str> = of_category.iter().copied().collect();
    (of_category.len(), amendments.len())
}

#[test]
fn should_count_amendments_and_give_the_rows_in_the_work_count_when_an_amendment_has_two_work_rows()
{
    assert_eq!(categories_of("7049142d0b26"), ["work", "work"]);
    let (rows, amendments) = rows_and_amendments("work");

    let text = residue(&[]);

    let expected = format!("{amendments} work ({rows} rows)");
    assert!(
        plain_lines(&text).contains(&expected),
        "a line {expected:?} in:\n{text}"
    );
}

#[test]
fn should_name_each_amendment_counted_in_two_categories_when_its_rows_fall_in_both() {
    let mut categories: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, category) in rows() {
        categories.entry(id).or_default().insert(category);
    }
    let in_two = categories.values().filter(|of| of.len() > 1).count();
    assert!(categories_of("c1213b7b65b7").contains(&"work"));
    assert!(categories_of("c1213b7b65b7").contains(&"unwritten"));

    let text = residue(&[]);

    let lines = plain_lines(&text);
    let expected = format!(
        "{in_two} amendment(s) have rows in more than one category, and each of those categories counts them:"
    );
    assert!(lines.contains(&expected), "a line {expected:?} in:\n{text}");
    assert!(
        lines.contains(&"c1213b7b65b7 work, unwritten".to_string()),
        "a line that names c1213b7b65b7 in:\n{text}"
    );
}

#[test]
fn should_say_that_an_amendment_can_have_a_row_for_each_item_when_residue_help_is_asked_for() {
    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["residue", "--help"])
        .output()
        .expect("the binary should run");
    assert!(output.status.success());

    let help = plain_lines(&String::from_utf8_lossy(&output.stdout)).join(" ");

    assert!(
        help.contains("One amendment can have one row for each item"),
        "{help}"
    );
}
