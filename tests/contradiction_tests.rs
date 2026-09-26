//! Contradictions: the subjects a dataset holds more than one link about.
//!
//! Two shapes, kept apart because they are different facts (#184):
//!
//! * **Duplication** — same subject, same object, links in more than one
//!   window. One method, run over two windows, placing one move twice.
//! * **Disagreement** — same subject, a different object. Two links that
//!   cannot both be true.
//!
//! The command **reports**. Decision 12 of #179 is settled: contradicting links
//! coexist, the contradiction is computed, and no link is stamped or rewritten.
//! Which link to keep is #172 and is deliberately left open, so nothing here
//! ranks by corroboration — #218 measured five pairs where the false link
//! scores higher, worst case 0.22 against 0.71.
//!
//! The other half of #184 is here too: a `redesignation-report` row must name
//! the window its link came from. Without it, two links for one statement come
//! out as rows identical but for the score, and a reader cannot see which window
//! made either. It needs the same corpus, so it is tested beside the rest.
//!
//! Every case is read out of the committed public law, `119-hr-1`, and the
//! committed release points. The two-window cases need the third release point,
//! `2025-08-14`, which is why it is vendored.

use std::collections::HashMap;
use std::process::Command;
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, adjacent_expressions, work_roots,
};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::parser::parse;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// The three release points the corpus holds, oldest first.
///
/// `119-hr-1` reached the Code between the first two. The third is here so the
/// corpus holds two adjacent windows: one window cannot duplicate anything.
const DATES: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// The one title `119-hr-1` renumbers most of.
///
/// Enough on its own for every question about links in one window, and cheap:
/// one title at one date is 56 MB of XML to parse.
const TITLE_26: [&str; 1] = ["usc26"];

/// Enough of the Code that fewer than twenty statements are left unplaced.
///
/// The human output prints the weakest twenty rows, and a statement nothing
/// placed is weaker than any placed row. With title 26 alone thirty statements
/// are unplaced, so all twenty printed rows are unplaced ones and no window is
/// ever printed. These five titles bring it to nineteen, which is the cheapest
/// set that does: titles 7 and 20 account for twelve of the thirty on their own.
const FIVE_TITLES: [&str; 5] = ["usc05", "usc07", "usc15", "usc20", "usc26"];

/// One title at one release point.
fn title(name: &str, date: &str) -> String {
    format!("tests/test_data/usc/{date}/{name}.xml")
}

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

/// Every title at each date named, then the bill, then the step that records
/// what the bill renumbered over every window the dataset holds.
///
/// The order a build takes (#181): the windows exist before the step runs over
/// them, because loading a bill records nothing.
fn dataset_over(dates: &[&str], titles: &[&str]) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for date in dates {
        for name in titles {
            let parsed = parse(&title(name, date), date).expect("the title should parse");
            for root in work_roots(parsed) {
                let work = WorkId::new(root.data.path.to_string());
                dataset
                    .add_expression(Expression {
                        id: ExpressionId::new(work, *date),
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

    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let windows = adjacent_expressions(&dataset).expect("the windows should list");
    dataset
        .record_redesignations_over(BILL_ID, &bill.root, &windows)
        .expect("the step should run");
    dataset
}

/// Write a dataset once and hand back its path, so the CLI tests below share
/// one build of it.
///
/// SQLite rather than compact JSON: a SQLite dataset is the one a command can
/// grow in place, so it is the form in which "no link was written" is worth
/// proving.
fn dataset_file(name: &str, dates: &[&str], titles: &[&str]) -> String {
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    dataset_over(dates, titles)
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// The committed corpus: two release points, so exactly one window.
fn one_window_file() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| dataset_file("contradictions_one_window.sqlite", &DATES[..2], &TITLE_26))
}

/// Three release points, so two adjacent windows, which is the least a
/// duplicated pair can be found in.
fn two_window_file() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| dataset_file("contradictions_two_windows.sqlite", &DATES, &FIVE_TITLES))
}

/// The two windows the three release points give.
fn both_windows() -> Vec<serde_json::Value> {
    DATES
        .windows(2)
        .map(|pair| serde_json::json!({"from_date": pair[0], "to_date": pair[1]}))
        .collect()
}

/// Every link a dataset file holds, serialized, over every kind it holds.
///
/// The records themselves rather than a count: a command that rewrote one link
/// and deleted another would leave the count where it was.
fn every_link_of(path: &str) -> String {
    let dataset = Dataset::open_sqlite(path).expect("the dataset should open");
    let mut links = Vec::new();
    for kind in dataset
        .count_links_by_kind()
        .expect("the kinds should count")
        .into_keys()
    {
        links.extend(dataset.links_by_kind(&kind).expect("the links should read"));
    }
    serde_json::to_string(&links).expect("the links should serialize")
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_report_nothing_when_the_dataset_holds_one_window() {
    // The negative case. One window cannot duplicate a link, and the command
    // must say so plainly rather than fail or stay silent about having looked.
    let output = run(&["contradictions", one_window_file(), "--json"]);

    assert!(
        output.status.success(),
        "contradictions should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the command should emit json");

    assert_eq!(
        report["duplication"].as_array().map(Vec::len),
        Some(0),
        "one window holds no duplicated pair, got:\n{report:#}"
    );
    assert_eq!(
        report["disagreement"].as_array().map(Vec::len),
        Some(0),
        "the corpus states no two objects for one subject, got:\n{report:#}"
    );

    // It says how much it read, so "nothing found" is distinguishable from
    // "nothing was looked at".
    assert!(
        report["totals"]["links_read"].as_u64().unwrap_or(0) > 0,
        "the command should report the links it read, got:\n{report:#}"
    );

    // Human output names the two categories, so a reader learns the command
    // checked both.
    let text = run(&["contradictions", one_window_file()]);
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.contains("Duplication") && text.contains("Disagreement"),
        "human output should name both categories, got:\n{text}"
    );
}

#[test]
fn should_find_and_categorise_the_duplicated_pairs_when_the_dataset_holds_two_windows() {
    // One method, run over two adjacent windows, places the same move twice.
    // Nothing surfaced that before this command (#218).
    let output = run(&["contradictions", two_window_file(), "--json"]);

    assert!(
        output.status.success(),
        "contradictions should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the command should emit json");

    let duplication = report["duplication"]
        .as_array()
        .expect("the report carries a duplication list");

    // A floor and not a total. #220 reports two statements asserting one move
    // collapsing into one stored link, so the figure moves when that is settled
    // and the shape does not.
    assert!(
        !duplication.is_empty(),
        "two windows over one bill should duplicate at least one pair, got:\n{report:#}"
    );
    assert_eq!(
        report["totals"]["duplication"].as_u64(),
        Some(duplication.len() as u64),
        "the total should count the groups reported"
    );

    // The two categories are separate lists, so a reader never has to work out
    // which fact a row states.
    assert!(
        report["disagreement"].is_array(),
        "disagreement should be its own list, got:\n{report:#}"
    );

    let windows = both_windows();
    for group in duplication {
        assert_eq!(
            group["kind"], "legislature.redesignated_as",
            "the corpus holds one link kind, and a group names it"
        );
        assert!(
            group["subject"]
                .as_str()
                .is_some_and(|name| !name.is_empty()),
            "a group names its subject, got:\n{group:#}"
        );

        let links = group["links"]
            .as_array()
            .expect("a group carries its links");
        assert!(
            links.len() >= 2,
            "duplication needs two links, got:\n{group:#}"
        );

        // Same subject, same object: that is what makes it duplication rather
        // than disagreement.
        let objects: std::collections::BTreeSet<&str> = links
            .iter()
            .filter_map(|link| link["object"].as_str())
            .collect();
        assert_eq!(
            objects.len(),
            1,
            "a duplication group holds one object, got:\n{group:#}"
        );

        // In more than one window, and each window is one the dataset holds.
        let named: std::collections::BTreeSet<String> = links
            .iter()
            .map(|link| link["window"].to_string())
            .collect();
        assert!(
            named.len() > 1,
            "duplication means more than one window, got:\n{group:#}"
        );
        for link in links {
            assert!(
                windows.contains(&link["window"]),
                "a link should name a window the dataset holds, got {}",
                link["window"]
            );
            // Method and version, so a reader can tell a statement this build
            // would make again from one it would not (#182).
            assert_eq!(
                link["method"], "amendingAction type=redesignate@1",
                "every link names its method with its version, got:\n{link:#}"
            );
            assert!(
                link["source"].as_str().is_some_and(|who| !who.is_empty()),
                "every link names what made it, got:\n{link:#}"
            );
        }
    }

    // Human output names the category, the windows and the method.
    let text = run(&["contradictions", two_window_file()]);
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    for date in DATES {
        assert!(
            text.contains(date),
            "human output should name the windows, got:\n{text}"
        );
    }
    assert!(
        text.contains("amendingAction type=redesignate@1"),
        "human output should name the method with its version, got:\n{text}"
    );
}

/// A contradicting link is named by its id, in both output forms.
///
/// Nothing printed a link id before this. A reader who found a contradiction
/// had no way to say which of the two links is wrong, because they could not
/// name either one to `settle` (#227). The id is a hash of what the link says,
/// so it is the same in every build of the dataset (ADR 0004).
#[test]
fn should_name_each_contradicting_link_by_its_id_when_a_contradiction_is_reported() {
    let output = run(&["contradictions", two_window_file(), "--json"]);
    assert!(
        output.status.success(),
        "contradictions should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the command should emit json");

    let duplication = report["duplication"]
        .as_array()
        .expect("the report carries a duplication list");
    assert!(
        !duplication.is_empty(),
        "the guard is worth nothing unless the command found something"
    );

    let mut named = Vec::new();
    for group in duplication {
        for link in group["links"]
            .as_array()
            .expect("a group carries its links")
        {
            let id = link["id"]
                .as_str()
                .unwrap_or_else(|| panic!("every link should carry its id, got:\n{link:#}"));
            assert_eq!(
                id.len(),
                words_to_data::review::ID_PREFIX_LENGTH,
                "one constant sets the printed length, got `{id}`"
            );
            assert!(
                id.chars().all(|letter| letter.is_ascii_hexdigit()),
                "an id is hexadecimal, got `{id}`"
            );
            named.push(id.to_string());
        }
    }

    // Two links in one group are two records, so two ids. A group that printed
    // one id twice would name neither link.
    let distinct: std::collections::BTreeSet<&String> = named.iter().collect();
    assert_eq!(
        distinct.len(),
        named.len(),
        "each contradicting link has its own id, got {named:?}"
    );

    // And the human output carries them too, so a reader at a terminal can
    // settle a link without piping through `--json`.
    //
    // The first group's links, because the human output prints a fixed number
    // of groups per category and `--json` carries them all.
    let shown: Vec<&str> = duplication[0]["links"]
        .as_array()
        .expect("a group carries its links")
        .iter()
        .filter_map(|link| link["id"].as_str())
        .collect();
    assert!(shown.len() >= 2, "duplication needs two links");

    let text = run(&["contradictions", two_window_file()]);
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    for id in &shown {
        assert!(
            text.contains(id),
            "human output should name link {id}, got:\n{text}"
        );
    }
    // And say what the id is for, because an id nobody can act on is noise.
    assert!(
        text.contains("words_to_data settle"),
        "human output should say how to settle a link it named, got:\n{text}"
    );
}

#[test]
fn should_leave_every_link_row_unchanged_when_the_command_runs() {
    // Decision 12 of #179: the contradiction is computed and never stamped. A
    // command that reported a duplicate and also resolved it would decide #172
    // by accident, and the decision is left open on purpose.
    //
    // Its own copy of the fixture, because the command must be the only thing
    // that touched the file between the two readings.
    let source = two_window_file();
    let path = format!(
        "{}/contradictions_read_only.sqlite",
        env!("CARGO_TARGET_TMPDIR")
    );
    std::fs::copy(source, &path).expect("the fixture should copy");

    let before = every_link_of(&path);
    assert!(!before.is_empty(), "the fixture should hold links to guard");

    let output = run(&["contradictions", &path, "--json"]);
    assert!(
        output.status.success(),
        "contradictions should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the command should emit json");
    assert!(
        !report["duplication"].as_array().expect("a list").is_empty(),
        "the guard is worth nothing unless the command found something"
    );

    let after = every_link_of(&path);
    assert_eq!(
        before.len(),
        after.len(),
        "the command should write no link, and the rows changed length"
    );
    assert!(
        before == after,
        "every link row should be byte-identical after the command ran"
    );
}

#[test]
fn should_name_the_window_its_link_came_from_when_a_redesignation_report_row_is_placed() {
    // Over two windows the report gives two rows for one statement, and until
    // now they were identical but for the score. The window is what tells them
    // apart, and the report already holds it: it is the two dates a link's
    // subject names.
    let output = run(&["redesignation-report", two_window_file(), "--json"]);
    assert!(
        output.status.success(),
        "the report should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the report should emit json");

    let rows = report["rows"].as_array().expect("the report carries rows");
    let windows = both_windows();
    let mut placed = 0;
    for row in rows {
        // Every field that was already there is still there, under its own name.
        for field in [
            "bill_id",
            "bill_path",
            "amendment_id",
            "clause",
            "reader",
            "placed",
            "reason",
            "from_path",
            "to_path",
            "corroboration",
        ] {
            assert!(
                row.get(field).is_some(),
                "the row should still carry {field}, got:\n{row:#}"
            );
        }

        if row["placed"] == serde_json::Value::Bool(true) {
            placed += 1;
            assert!(
                windows.contains(&row["window"]),
                "a placed row should name a window the dataset holds, got:\n{row:#}"
            );
        } else {
            // Nothing placed it, so there is no window it came from. Naming one
            // would invent it.
            assert_eq!(
                row["window"],
                serde_json::Value::Null,
                "an unplaced row names no window, got:\n{row:#}"
            );
        }
    }
    assert!(
        placed > 0,
        "the two-window corpus should place rows for the window to name"
    );

    // Two rows for one statement are now distinguishable by more than a score.
    let mut seen = std::collections::BTreeSet::new();
    for row in rows
        .iter()
        .filter(|row| row["placed"] == serde_json::Value::Bool(true))
    {
        assert!(
            seen.insert((
                row["amendment_id"].to_string(),
                row["from_path"].to_string(),
                row["to_path"].to_string(),
                row["window"].to_string(),
            )),
            "a placed row should be told apart by its window, got:\n{row:#}"
        );
    }

    // And the human output names it, not only `--json`.
    let text = run(&["redesignation-report", two_window_file()]);
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(
        text.contains(&format!("{} -> {}", DATES[0], DATES[1]))
            || text.contains(&format!("{} -> {}", DATES[1], DATES[2])),
        "human output should name the window a row came from, got:\n{text}"
    );
}

/// Real annotations from one matching run over the real corpus.
///
/// 530 paths, and **132 of them carry more than one amendment**. Every
/// annotation names the same annotator, `model:deepseek-v4-pro`, so the whole
/// fixture is the work of **one maker**.
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";

/// The work most of the fixture's annotations belong to.
const TITLE_26_WORK: &str = "uscode/title_26";

/// Every fixture annotation whose paths all sit under title 26, stored as links
/// over one window.
///
/// One work, one window, one annotator: whatever this dataset holds, it holds
/// one maker's single answer.
fn one_makers_annotations() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<words_to_data::annotation::ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");

    let from = ExpressionId::new(WorkId::new(TITLE_26_WORK), "2025-07-18");
    let to = ExpressionId::new(WorkId::new(TITLE_26_WORK), "2025-07-30");

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "One matching run".to_string(),
        ..Default::default()
    });
    for annotation in &annotations {
        if annotation.paths.is_empty()
            || !annotation
                .paths
                .iter()
                .all(|path| path.starts_with(TITLE_26_WORK))
        {
            continue;
        }
        for link in words_to_data::link::Link::from_annotation(annotation, &from, &to) {
            dataset.add_link(link).expect("the link should be added");
        }
    }
    dataset
}

#[test]
fn should_report_no_disagreement_when_one_maker_names_several_amendments_at_one_provision() {
    let dataset = one_makers_annotations();

    // Guard, so the assertion below cannot pass on an empty dataset: the
    // fixture must really hold a provision that two amendments changed.
    let mut by_subject: HashMap<String, usize> = HashMap::new();
    for link in dataset
        .links_by_kind("legislature.amended_by")
        .expect("links should be readable")
    {
        *by_subject.entry(link.subject.name()).or_default() += 1;
    }
    let shared = by_subject.values().filter(|count| **count > 1).count();
    assert!(
        shared > 0,
        "the fixture should hold provisions changed by more than one amendment, found none"
    );

    let report =
        words_to_data::inspect::contradictions(&dataset).expect("contradictions should be read");

    // A provision changed by several amendments of one bill is ordinary law, not
    // a contradiction. "Sections 1202(b)(2), 1202(g)(2)(A), and 1202(j)(1)(A)
    // are each amended by striking ..." is one instruction with three targets.
    // A contradiction needs two *makers* answering one question differently,
    // and this dataset holds one.
    assert_eq!(
        report.disagreement.len(),
        0,
        "one maker cannot disagree with itself: {} provision(s) are shared by \
         several amendments and none of them is a contradiction",
        shared
    );
}

/// The same real annotations, stored over **both** windows the corpus holds.
///
/// One annotator over two windows is two makers, and two makers is what it takes
/// for a group to be reported at all. Matching a dataset over both of its windows
/// is an ordinary thing to do with a three-release-point corpus.
fn two_makers_annotations() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<words_to_data::annotation::ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Two matching runs".to_string(),
        ..Default::default()
    });
    for (from_date, to_date) in [(DATES[0], DATES[1]), (DATES[1], DATES[2])] {
        let from = ExpressionId::new(WorkId::new(TITLE_26_WORK), from_date);
        let to = ExpressionId::new(WorkId::new(TITLE_26_WORK), to_date);
        for annotation in &annotations {
            if annotation.paths.is_empty()
                || !annotation
                    .paths
                    .iter()
                    .all(|path| path.starts_with(TITLE_26_WORK))
            {
                continue;
            }
            for link in words_to_data::link::Link::from_annotation(annotation, &from, &to) {
                dataset.add_link(link).expect("the link should be added");
            }
        }
    }
    dataset
}

#[test]
fn should_name_the_change_a_link_records_when_its_object_is_an_amendment() {
    let dataset = two_makers_annotations();

    let report =
        words_to_data::inspect::contradictions(&dataset).expect("contradictions should be read");
    let groups: Vec<_> = report
        .duplication
        .iter()
        .chain(report.disagreement.iter())
        .collect();
    assert!(
        !groups.is_empty(),
        "two makers over one subject should report at least one group"
    );

    // An amendment can make several changes at one provision, and the store keeps
    // those apart because a link is identified by what it says. The amendment's
    // reference alone does not say what a link records, so two of them read as one
    // row repeated. Measured on a real dataset: three links at
    // `section_3839bb-5/subsection_f/paragraph_1`, two naming one amendment, and
    // with no corroboration to tell them apart they printed identically.
    for group in groups {
        for link in &group.links {
            if !link.object.starts_with("legislature.amendment:") {
                continue;
            }
            let rendered = serde_json::to_value(link).expect("a link should serialise");
            let change = rendered.get("change").and_then(|value| value.as_str());
            assert!(
                change.is_some_and(|text| !text.is_empty()),
                "a reported amendment link should name the change it records, \
                 got {rendered} for {}",
                group.subject
            );
        }
    }
}
