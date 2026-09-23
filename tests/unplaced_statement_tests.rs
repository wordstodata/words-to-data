//! An unplaced statement: a redesignation the corpus states and no reader could
//! place, kept where a party reading the dataset can find it.
//!
//! `CONTEXT.md` defines the concept. It carries the words, the reason, the
//! reader that failed, and the path in the source document where the words sit,
//! so a reviewer can open them. Until #196 the dataset did not hold the bill, so
//! there was no path to give. It holds the bill now.
//!
//! Every case here is read out of the committed public law, `119-hr-1`, and the
//! committed release points.

use std::collections::{BTreeMap, HashMap};
use std::process::Command;
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId, adjacent_expressions,
    work_roots,
};
use words_to_data::inspect;
use words_to_data::legislature::redesignation::{Reader, RedesignationReport};
use words_to_data::link::LinkKind;
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::bill_redesignation::redesignations_stated_in;
use words_to_data::uslm::parser::parse;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// The bill as the Congress client would hand it over, read from the committed
/// cache, so a test exercises the path a build really takes.
fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: HashMap::new(),
    }
}

/// A dataset holding the bill and nothing else.
fn dataset_holding_the_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_name_the_path_in_the_bill_when_a_statement_is_read_from_the_stored_bill() {
    let dataset = dataset_holding_the_bill();
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");

    let stated = redesignations_stated_in(BILL_ID, &bill.root);
    assert_eq!(stated.len(), 57, "119-hr-1 states 57 redesignations");

    // Each statement says where in the bill its words sit, and the path leads
    // back to a node the dataset holds. That is what a reviewer opens.
    for statement in &stated {
        let path = statement
            .path
            .as_deref()
            .expect("a statement read from the stored bill knows where it sat");
        assert!(
            bill.root.find(path).is_some(),
            "the bill should hold a node at {path}"
        );
    }
}

/// Read the bill out of the dataset and record what it renumbered, over every
/// window the dataset holds.
///
/// The explicit step (#181), as `build-dataset` runs it after it has loaded
/// everything. Loading the bill records nothing.
fn record_over_every_window(dataset: &mut Dataset<InMemoryStorage>) -> RedesignationReport {
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let windows = adjacent_expressions(dataset).expect("the windows should list");
    dataset
        .record_redesignations_over(BILL_ID, &bill.root, &windows)
        .expect("the step should run")
}

/// Title 26 at both release points, then the bill, then the step that records
/// what the bill renumbered.
///
/// The order and the work a build does (#181): the windows exist before the
/// step runs over them, and the step is run because loading a bill records
/// nothing. The report is what a build prints.
fn dataset_built_over_title_26() -> (Dataset<InMemoryStorage>, RedesignationReport) {
    let mut dataset = dataset_holding_title_26_and_the_bill();
    let report = record_over_every_window(&mut dataset);
    (dataset, report)
}

/// Title 26 at both release points, and then the bill, with no step run over
/// them.
fn dataset_holding_title_26_and_the_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (path, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        let parsed = parse(path, date).expect("title 26 should parse");
        for root in work_roots(parsed) {
            let work = WorkId::new(root.data.path.to_string());
            dataset
                .add_expression(Expression {
                    id: ExpressionId::new(work, date),
                    label: None,
                    root,
                })
                .expect("the expression should store");
        }
    }
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_name_the_reader_and_the_path_when_a_statement_cannot_be_placed() {
    let (dataset, report) = dataset_built_over_title_26();
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");

    assert!(
        !report.unplaced.is_empty(),
        "119-hr-1 states more than title 26 alone can place"
    );
    for statement in &report.unplaced {
        // The rule reader is the only one that reads today. The model reader
        // comes with #154 (ADR 0010).
        assert_eq!(statement.reader, Reader::Rule);

        // The words, and where they sit in the bill, so a reviewer can open
        // them. Most of what the corpus leaves unplaced has no US Code path at
        // all, so this is the only path it carries.
        assert!(!statement.text.is_empty());
        let path = statement
            .path
            .as_deref()
            .expect("an unplaced statement says where its words sit");
        assert!(
            bill.root.find(path).is_some(),
            "the bill should hold a node at {path}"
        );
    }
}

#[test]
fn should_put_the_unplaced_statements_first_when_it_reports_a_bill() {
    let (dataset, _) = dataset_built_over_title_26();

    // The whole report, from the dataset alone: no XML, and no model call.
    let report =
        inspect::redesignation_report(&dataset, None).expect("the report should read the dataset");

    // One row for each link, and one row for each statement no reader placed.
    // The two counts measure different things and are never added (#166).
    assert!(report.totals.statements > 0);
    assert!(report.totals.links > 0);
    assert!(report.totals.unplaced > 0);
    assert_eq!(
        report.rows.len(),
        report.totals.links + report.totals.unplaced
    );

    // Weakest first: nothing placed at all comes before anything placed.
    let first_placed = report
        .rows
        .iter()
        .position(|row| row.placed)
        .expect("title 26 places some of what the bill states");
    assert!(
        report.rows[..first_placed].iter().all(|row| !row.placed),
        "every unplaced row comes before the first placed one"
    );
    assert_eq!(first_placed, report.totals.unplaced);

    // Then the placed rows run from the least corroborated upwards, so a
    // reviewer reads the doubtful handful first (ADR 0010).
    let figures: Vec<f32> = report.rows[first_placed..]
        .iter()
        .map(|row| row.corroboration.expect("a placed row carries a figure"))
        .collect();
    assert!(
        figures.windows(2).all(|pair| pair[0] <= pair[1]),
        "placed rows run from the weakest figure upwards: {figures:?}"
    );

    // Each unplaced row says which bill, where in it, which amendment, the
    // words, which reader failed, and why.
    for row in &report.rows[..first_placed] {
        assert_eq!(row.bill_id, BILL_ID);
        assert!(row.bill_path.is_some());
        assert!(!row.amendment_id.is_empty());
        assert!(!row.clause.is_empty());
        assert_eq!(row.reader, Reader::Rule);
        assert!(row.reason.is_some());
        assert_eq!(row.from_path, None);
        assert_eq!(row.to_path, None);
    }

    // Each placed row names the two paths the provision moved between, and
    // states no reason, because nothing failed.
    for row in &report.rows[first_placed..] {
        assert!(row.from_path.is_some());
        assert!(row.to_path.is_some());
        assert_eq!(row.reason, None);
    }
}

#[test]
fn should_report_every_statement_as_unplaced_when_the_dataset_holds_no_window() {
    // The organic order: a bill enters a dataset before the release points it
    // amends. There is nothing to check a statement against, so nothing can be
    // placed — and the report must say so, rather than say nothing.
    let dataset = dataset_holding_the_bill();

    let report =
        inspect::redesignation_report(&dataset, None).expect("the report should read the dataset");

    assert_eq!(report.totals.statements, 57);
    assert_eq!(report.totals.links, 0);
    assert_eq!(report.totals.unplaced, 57);
    assert_eq!(report.rows.len(), 57);
    assert!(report.rows.iter().all(|row| !row.placed));

    // One reason, and it names what is missing: the windows, not the words.
    assert_eq!(
        report.totals.reasons,
        std::collections::BTreeMap::from([(
            "the dataset holds no window to check the statement against".to_string(),
            57
        )])
    );
}

/// A dataset file holding the bill, written once for the CLI tests below.
fn bill_dataset_file() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/unplaced_bill.json", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        dataset_holding_the_bill()
            .save(&path, Format::Compact)
            .expect("the fixture should save");
        path
    })
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_emit_one_row_for_each_statement_when_the_report_command_runs_with_json() {
    let output = run(&["redesignation-report", bill_dataset_file(), "--json"]);

    assert!(
        output.status.success(),
        "the report should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the report should emit json");

    assert_eq!(report["totals"]["bills"], 1);
    assert_eq!(report["totals"]["statements"], 57);
    assert_eq!(report["totals"]["links"], 0);
    assert_eq!(report["totals"]["unplaced"], 57);

    let rows = report["rows"].as_array().expect("the report carries rows");
    assert_eq!(rows.len(), 57);

    // Every field an agent reads to decide what to work on next.
    assert_eq!(rows[0]["bill_id"], BILL_ID);
    assert_eq!(rows[0]["reader"], "rule");
    assert_eq!(rows[0]["placed"], false);
    assert!(rows[0]["bill_path"].is_string());
    assert!(rows[0]["amendment_id"].is_string());
    assert!(rows[0]["clause"].is_string());
    assert!(rows[0]["reason"].is_string());
}

#[test]
fn should_read_one_bill_when_the_report_command_is_given_a_bill_id() {
    let output = run(&[
        "redesignation-report",
        bill_dataset_file(),
        "--bill-id",
        BILL_ID,
        "--json",
    ]);

    assert!(output.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the report should emit json");
    assert_eq!(report["totals"]["bills"], 1);

    // A bill this dataset does not hold reports nothing, rather than failing.
    // An empty answer is the truthful one: there is no document to read.
    let missing = run(&[
        "redesignation-report",
        bill_dataset_file(),
        "--bill-id",
        "119-hr-999",
        "--json",
    ]);
    assert!(missing.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&missing.stdout).expect("the report should emit json");
    assert_eq!(report["totals"]["bills"], 0);
    assert_eq!(report["rows"].as_array().map(Vec::len), Some(0));
}

#[test]
fn should_carry_one_line_of_renumbering_counts_when_info_runs() {
    // `info` says how much is unplaced; the report says which and why. Without
    // the line, a reader has no sign that the detail is worth asking for.
    let text = run(&["info", bill_dataset_file()]);
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.contains("Renumbering: 57 statement(s), 0 link(s), 57 not placed"),
        "info should carry one line of counts, got:\n{text}"
    );

    let output = run(&["info", bill_dataset_file(), "--json"]);
    let info: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("info --json should emit json");
    assert_eq!(info["redesignations"]["statements"], 57);
    assert_eq!(info["redesignations"]["links"], 0);
    assert_eq!(info["redesignations"]["unplaced"], 57);
}

/// The seven titles `119-hr-1` renumbers provisions in.
///
/// The same seven `redesignation_tests` sweeps, so the two tests measure one
/// corpus. The whole Code gives 89 links and 13 unplaced statements; these
/// seven give 80 and 17, because six of the sections under amendment sit in
/// titles nobody put in this list.
const TITLES_NAMED: [&str; 7] = [
    "usc05.xml",
    "usc07.xml",
    "usc10.xml",
    "usc15.xml",
    "usc20.xml",
    "usc26.xml",
    "usc42.xml",
];

#[test]
fn should_show_a_row_for_every_statement_when_it_reports_the_corpus() {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for file in TITLES_NAMED {
        for date in [BEFORE, AFTER] {
            let parsed = parse(&format!("tests/test_data/usc/{date}/{file}"), date)
                .expect("it should parse");
            for root in work_roots(parsed) {
                let work = WorkId::new(root.data.path.to_string());
                dataset
                    .add_expression(Expression {
                        id: ExpressionId::new(work, date),
                        label: None,
                        root,
                    })
                    .expect("the expression should store");
            }
        }
    }
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    // What `build-dataset` does after it has loaded everything (#181). The
    // count below is the count this corpus gave when loading recorded the links
    // itself, so it says the explicit step loses nothing.
    record_over_every_window(&mut dataset);

    let report =
        inspect::redesignation_report(&dataset, None).expect("the report should read the dataset");

    // The numbers `redesignation_tests` fixes for this same corpus, read back
    // out of the dataset rather than out of the bill's XML.
    assert_eq!(report.totals.bills, 1);
    assert_eq!(report.totals.statements, 57);
    assert_eq!(report.totals.unplaced, 17);

    // 80 links, where the resolver made 81 renumberings. A link is identified
    // by what it says, so two statements that renumber one provision the same
    // way between the same two dates are one link
    // (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    // The report counts what the dataset holds, so it says 80.
    let held = dataset
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read");
    assert_eq!(held.len(), 80);
    assert_eq!(report.totals.links, 80);

    // Not one statement is dropped. 80 placed renumberings and 17 statements
    // nothing placed, each of them a row an agent can act on.
    assert_eq!(report.rows.len(), 97);
    assert_eq!(report.rows.iter().filter(|row| !row.placed).count(), 17);

    // Every reason is a phrase that says what to do next, and the counts add up
    // to the statements nothing placed.
    assert_eq!(
        report.totals.reasons.values().sum::<usize>(),
        report.totals.unplaced
    );

    // The reasons a live sweep of `119-hr-1` gives, grouped as the issue lists
    // them. Each one names something a reader can act on; "could not parse"
    // alone would tell a maintainer nothing about which bills to look at.
    let mut grouped: BTreeMap<&str, usize> = BTreeMap::new();
    for (reason, count) in &report.totals.reasons {
        let group = if reason.starts_with("no provision at") {
            "no provision at <path> before/after the bill"
        } else if reason.starts_with("nothing says which title") {
            "nothing says which title holds section 4 / 101"
        } else if reason.ends_with("names more than one provision") {
            "names more than one provision"
        } else {
            reason.as_str()
        };
        *grouped.entry(group).or_default() += count;
    }
    assert_eq!(
        grouped,
        BTreeMap::from([
            ("no section under amendment was named", 3),
            ("nothing says which title holds section 4 / 101", 4),
            ("a table of sections, not a provision", 3),
            ("no provision at <path> before/after the bill", 5),
            ("the numbers of a paragraph do not run in a known series", 1),
            ("names more than one provision", 1),
        ])
    );

    // Each unplaced row leads back to the words in the bill that defeated the
    // reader. That is the whole point of the path.
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill");
    for row in report.rows.iter().filter(|row| !row.placed) {
        let path = row
            .bill_path
            .as_deref()
            .expect("an unplaced row has a path");
        assert!(bill.root.find(path).is_some(), "the bill holds {path}");
    }
}

#[test]
fn should_call_a_statement_unplaced_when_the_dataset_holds_no_link_for_it() {
    // The organic order the other way round: the bill arrives first, and the
    // release points it amends arrive after it. Nothing re-sweeps, so the
    // dataset holds windows and no link at all.
    let mut dataset = dataset_holding_the_bill();
    for (path, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        let parsed = parse(path, date).expect("title 26 should parse");
        for root in work_roots(parsed) {
            let work = WorkId::new(root.data.path.to_string());
            dataset
                .add_expression(Expression {
                    id: ExpressionId::new(work, date),
                    label: None,
                    root,
                })
                .expect("the expression should store");
        }
    }

    let report =
        inspect::redesignation_report(&dataset, None).expect("the report should read the dataset");

    // The report says what the dataset holds. It holds no link, so nothing is
    // placed — a report that resolved the statements afresh would claim 49
    // links this dataset does not have.
    assert_eq!(report.totals.statements, 57);
    assert_eq!(report.totals.links, 0);
    assert_eq!(report.totals.unplaced, 57);
    assert!(report.rows.iter().all(|row| !row.placed));

    // And the reason tells the two cases apart. A statement this build can
    // place is waiting for a step that has not run; the rest cannot be placed
    // at all, and say why.
    // 26 of the 57 are statements title 26 can place. The other 31 cannot be
    // placed against title 26 alone, and keep their own reasons.
    let waiting = "the dataset holds no link for a statement this build can place";
    assert_eq!(report.totals.reasons.get(waiting), Some(&26));
    assert!(
        report.totals.reasons.len() > 1,
        "the statements title 26 cannot place keep their own reasons: {:?}",
        report.totals.reasons
    );
}
