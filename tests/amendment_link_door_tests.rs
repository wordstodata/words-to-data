//! `link-amendment`: the door through which an agent records an `amended_by`
//! link it found, and which refuses a path the diff did not produce.
//!
//! The matcher left 168 of 603 amendments of `119-hr-1` unlinked, and an agent
//! that resolved 20 of them with the CLI alone could record none (#249). This
//! door lets it record them without letting it invent a path: every path must
//! have changed in the window it names
//! (`docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md`).
//!
//! Every case reads the committed corpus: title 26 at two release points and
//! the committed public law. No model runs and nothing is mocked.

use std::process::Command;
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, Format};

/// The committed public law.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const FROM: &str = "uscode/title_26@2025-07-18";
const TO: &str = "uscode/title_26@2025-07-30";
/// The release point after that, so the corpus holds a window that skips one.
const TITLE_26_LATER: &str = "tests/test_data/usc/2025-08-14/usc26.xml";
const LATER: &str = "uscode/title_26@2025-08-14";

/// The amendment that inserts a new § 174A after § 174.
const INSERTS_174A: &str = "2841e731435d7fb5b093e5bd6a39be845b1f565418a78949b0d8e0820ac2e62b";
/// The section that amendment adds, which the diff reports as added.
const SECTION_174A: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A";

/// The bill as the Congress client would hand it over, read from the committed
/// cache.
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
        member_jsons: std::collections::HashMap::new(),
    }
}

/// Title 26 at both release points and the public law, saved once as a compact
/// dataset that every case reads and none writes over.
fn fixture() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        for (file, date) in [
            (TITLE_26_BEFORE, "2025-07-18"),
            (TITLE_26_AFTER, "2025-07-30"),
            (TITLE_26_LATER, "2025-08-14"),
        ] {
            dataset
                .add_uslm_xml(file, date, None)
                .expect("title 26 should load");
        }
        dataset
            .load_bill_download(&committed_bill_download())
            .expect("the bill should load");
        let path = format!("{}/amendment_link_door.json", env!("CARGO_TARGET_TMPDIR"));
        dataset
            .save(&path, Format::Compact)
            .expect("the fixture should save");
        path
    })
}

/// A place for one case to write its result.
fn output_for(case: &str) -> String {
    let path = format!("{}/{case}.json", env!("CARGO_TARGET_TMPDIR"));
    // A result left by an earlier run must not pass for this run's result.
    let _ = std::fs::remove_file(&path);
    path
}

/// One run of the binary.
fn run(command: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg(command)
        .args(args)
        .output()
        .expect("the binary should run")
}

/// The rows `annotations --json` lists for the bill.
fn annotation_rows(dataset: &str) -> Vec<serde_json::Value> {
    let output = run("annotations", &[dataset, "--bill", BILL_ID, "--json"]);
    assert!(
        output.status.success(),
        "the listing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    json["annotations"]
        .as_array()
        .expect("an annotations array")
        .clone()
}

#[test]
fn should_list_the_recorded_link_with_its_id_when_an_agent_records_a_path_that_changed() {
    let written = output_for("door_records_174a");

    let output = run(
        "link-amendment",
        &[
            fixture(),
            "--bill",
            BILL_ID,
            "--amendment",
            INSERTS_174A,
            "--from",
            FROM,
            "--to",
            TO,
            "--path",
            SECTION_174A,
            "--source",
            "agent:claude",
            "--reason",
            "The amendment inserts a new section 174A after section 174, and the diff adds it.",
            "--output",
            &written,
        ],
    );

    assert!(
        output.status.success(),
        "the link should record, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = annotation_rows(&written);
    assert_eq!(rows.len(), 1, "one link was recorded: {rows:#?}");
    let row = &rows[0];
    assert_eq!(row["amendment_id"], INSERTS_174A);
    assert_eq!(row["paths"][0], SECTION_174A);
    assert_eq!(row["annotator"], "agent:claude");
    // The markup states `amend` and `insert`; `insert` is what it does.
    assert_eq!(row["operation"], "insert");
    assert!(
        row["link_ids"][0].as_str().is_some_and(|id| !id.is_empty()),
        "the row names the link it lists: {row:#?}"
    );
}

/// A link is identified by what it says, so an agent that records the same
/// finding twice leaves one link
/// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
#[test]
fn should_store_the_link_once_when_the_same_link_is_recorded_twice() {
    let once = output_for("door_records_once");
    let twice = output_for("door_records_twice");
    let reason = "The amendment inserts a new section 174A after section 174.";

    let first = link_amendment(&recording(SECTION_174A, reason, &once));
    assert!(first.status.success(), "the first record should write");
    let mut again = recording(SECTION_174A, reason, &twice);
    again[0] = once.clone();
    let second = link_amendment(&again);
    assert!(
        second.status.success(),
        "the second record should write, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let rows = annotation_rows(&twice);
    assert_eq!(rows.len(), 1, "one annotation: {rows:#?}");
    assert_eq!(
        rows[0]["link_ids"].as_array().map(Vec::len),
        Some(1),
        "and one link under it: {rows:#?}"
    );
}

/// A recorded link enters the review loop unchanged: `settle` reads it by the id
/// `annotations` prints, and shows the reason the recorder wrote
/// (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
#[test]
fn should_settle_the_recorded_link_when_a_reviewer_names_it_by_its_id() {
    let recorded = output_for("door_records_for_settle");
    let settled = output_for("door_recorded_and_settled");
    let reason = "The amendment inserts a new section 174A after section 174.";
    let output = link_amendment(&recording(SECTION_174A, reason, &recorded));
    assert!(output.status.success(), "the link should record");
    let rows = annotation_rows(&recorded);
    let id = rows[0]["link_ids"][0]
        .as_str()
        .expect("the row names its link")
        .to_string();

    let explained = run("settle", &[&recorded, "--link", &id, "--explain"]);
    assert!(explained.status.success(), "--explain should read the link");
    let said = String::from_utf8_lossy(&explained.stdout);
    assert!(
        said.contains("agent:claude") && said.contains(SECTION_174A),
        "settle shows who recorded the link and where it points: {said}"
    );

    let output = run(
        "settle",
        &[
            &recorded,
            "--link",
            &id,
            "--verdict",
            "confirmed",
            "--reviewer",
            "human:jesse",
            "--reason",
            "Section 174A is new in this window, and this amendment enacts it.",
            "--output",
            &settled,
        ],
    );
    assert!(
        output.status.success(),
        "the recorded link should settle, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Recorded a review"),
        "settle records a review of it"
    );
}

/// § 61, which the corpus holds at both ends of the window and which did not
/// change in it.
const SECTION_61: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_I/section_61";

/// The arguments of a run that records `path` for the § 174A amendment.
fn recording(path: &str, reason: &str, written: &str) -> Vec<String> {
    [
        fixture(),
        "--bill",
        BILL_ID,
        "--amendment",
        INSERTS_174A,
        "--from",
        FROM,
        "--to",
        TO,
        "--path",
        path,
        "--source",
        "agent:claude",
        "--reason",
        reason,
        "--output",
        written,
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect()
}

/// Run the door with owned arguments.
fn link_amendment(args: &[String]) -> std::process::Output {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run("link-amendment", &args)
}

#[test]
fn should_refuse_and_write_nothing_when_a_path_did_not_change_in_the_window() {
    let written = output_for("door_refuses_unchanged");

    let output = link_amendment(&recording(
        SECTION_61,
        "An agent that guessed a section the amendment did not touch.",
        &written,
    ));

    assert!(
        !output.status.success(),
        "a path that did not change must be refused"
    );
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains(SECTION_61) && said.contains("did not change"),
        "the refusal names the path and what failed: {said}"
    );
    assert!(
        !std::path::Path::new(&written).exists(),
        "a refusal writes nothing"
    );
}

#[test]
fn should_refuse_and_say_so_when_a_path_exists_at_neither_end_of_the_window() {
    let written = output_for("door_refuses_missing_path");
    let missing = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174Z";

    let output = link_amendment(&recording(
        missing,
        "A path an agent typed that the Code does not hold.",
        &written,
    ));

    assert_eq!(output.status.code(), Some(1), "a missing path is refused");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains(missing) && said.contains("does not exist"),
        "the refusal says the path does not exist, which is not the same as unchanged: {said}"
    );
    assert!(
        !std::path::Path::new(&written).exists(),
        "a refusal writes nothing"
    );
}

/// A window that skips a release point is two windows, and a change in it
/// cannot be placed in either. § 174A was added in the first, so it also
/// differs across the pair that skips, and only the window check refuses it.
#[test]
fn should_refuse_and_write_nothing_when_the_window_skips_a_release_point() {
    let written = output_for("door_refuses_skipping_window");
    let mut args = recording(
        SECTION_174A,
        "The right section over a window that is two windows.",
        &written,
    );
    let at = args
        .iter()
        .position(|arg| arg == TO)
        .expect("the run names the later end");
    args[at] = LATER.to_string();

    let output = link_amendment(&args);

    assert_eq!(
        output.status.code(),
        Some(1),
        "a skipping window is refused"
    );
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains(TO) && said.contains("adjacent"),
        "the refusal says the window is not adjacent, and names the expression between: {said}"
    );
    assert!(
        !std::path::Path::new(&written).exists(),
        "a refusal writes nothing"
    );
}

#[test]
fn should_refuse_and_write_nothing_when_the_bill_holds_no_such_amendment() {
    let written = output_for("door_refuses_unknown_amendment");
    let mut args = recording(
        SECTION_174A,
        "An amendment id the bill does not hold.",
        &written,
    );
    let unknown = "0".repeat(64);
    let at = args
        .iter()
        .position(|arg| arg == INSERTS_174A)
        .expect("the run names the amendment");
    args[at] = unknown.clone();

    let output = link_amendment(&args);

    assert_eq!(
        output.status.code(),
        Some(1),
        "an unknown amendment is refused, not a crash"
    );
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains(&unknown) && said.contains(BILL_ID),
        "the refusal names the amendment and the bill: {said}"
    );
    assert!(
        !std::path::Path::new(&written).exists(),
        "a refusal writes nothing"
    );
}

#[test]
fn should_refuse_and_write_nothing_when_the_reason_is_empty() {
    let written = output_for("door_refuses_no_reason");

    let output = link_amendment(&recording(SECTION_174A, "   ", &written));

    assert!(!output.status.success(), "an empty reason must be refused");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains("reason"),
        "the refusal says the reason is what failed: {said}"
    );
    assert!(
        !std::path::Path::new(&written).exists(),
        "a refusal writes nothing"
    );
}
