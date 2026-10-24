//! Where the address of a *"Such section"* amendment comes from (#364).
//!
//! Pub. L. 119-60 § 1702(b) says *"Such section is further amended, in
//! subsection (c)—"*. The markup gave it no section before #364, and only the
//! OLRC's classification table gave it one, 17 U.S.C. 105, with nothing
//! below. Now the bill's markup gives the address, and the table does not.
//!
//! The case reads the committed corpus: the public law `119-s-1071`, title 17
//! of the Code at its three committed release points, and the committed OLRC
//! classification table stated as links, as `add-classifications` states
//! them. No model runs and nothing is mocked.

use std::path::PathBuf;
use std::process::Command;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::uslm::bill_parser::{bill_expression, bill_of_document};

const BILL_ID: &str = "119-s-1071";
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// The bill and title 17 at each committed release point, with the committed
/// classification table as links, saved as a database.
///
/// The cache holds the law's markup and not its metadata, so the bill is
/// loaded as `load_bill_download` loads it, without the sponsors.
fn built() -> PathBuf {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    let xml = std::fs::read_to_string(
        "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml",
    )
    .expect("the public law should be committed");
    let markup = roxmltree::Document::parse(&xml).expect("the public law should parse");
    let (bill, _) = bill_of_document(&markup, BILL_ID);
    dataset.add_bill(bill).expect("the bill should add");
    let (document, _) = bill_expression(&markup, BILL_ID).expect("the bill should read");
    dataset
        .add_expression(document)
        .expect("the bill's document should add");
    for date in RELEASE_POINTS {
        dataset
            .add_uslm_xml(&format!("tests/test_data/usc/{date}/usc17.xml"), date, None)
            .expect("the title should parse");
    }

    let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
        .expect("the committed table should read");
    let table = ClassificationTable::parse(&html).expect("the committed table should parse");
    let mut paths = SectionPaths::new();
    for date in RELEASE_POINTS {
        let expression = dataset
            .get_expression(&ExpressionId::new(WorkId::new("uscode/title_17"), date))
            .expect("storage should answer")
            .expect("the title is held");
        paths.add_work(&expression.root);
    }
    let scope = dataset.scope().expect("the scope should derive");
    for link in classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links {
        dataset.add_link(link).expect("the link should add");
    }

    let path = PathBuf::from(format!(
        "{}/such_section_address_source.sqlite",
        env!("CARGO_TARGET_TMPDIR")
    ));
    let _ = std::fs::remove_file(&path);
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// Pub. L. 119-60 § 1702(b)(1)–(4) act in 17 U.S.C. 105(c)(1)–(4). The bill's
/// markup gives that address, so what `link-by-evidence` records says the
/// address came from the bill, and not from the OLRC's table.
#[test]
fn should_take_the_address_from_the_bill_and_not_the_olrc_when_the_line_says_such_section() {
    let dataset = built();
    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "residue",
            dataset.to_str().expect("a UTF-8 path"),
            "--bill",
            BILL_ID,
            "--json",
        ])
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "residue should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    let rows = json["rows"].as_array().expect("a rows array");

    for paragraph in 1..=4 {
        let path = format!(
            "publiclawdocument_119-60/division_A/title_XVII/section_1702/subsection_b/paragraph_{paragraph}"
        );
        let row = rows
            .iter()
            .find(|row| row["address"]["path"] == path.as_str())
            .unwrap_or_else(|| panic!("§ 1702(b)({paragraph}) is listed"));
        assert_eq!(row["address_source"], "markup", "the row: {row}");
        assert_eq!(
            row["address"]["section"], "/us/usc/t17/s105",
            "the row: {row}"
        );
        let steps: Vec<&str> = row["address"]["container"]
            .as_array()
            .expect("a list of steps")
            .iter()
            .map(|step| step["number"].as_str().expect("a number"))
            .collect();
        assert_eq!(steps, ["c", &paragraph.to_string()], "the row: {row}");
    }
}
