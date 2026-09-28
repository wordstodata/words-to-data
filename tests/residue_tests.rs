//! `residue`: every amendment of a public law that no standing `amended_by`
//! link names, with the stage and the reason it stopped (#251).
//!
//! Stage 5 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//! The list is derived every time it is asked for and never stored
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! Every case reads the committed corpus: the public law `119-hr-1` and titles 7
//! and 26 of the Code at its three committed release points, built the way
//! `build-dataset` builds it and then linked by `link-by-evidence`. No model
//! runs and nothing is mocked.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, ExpressionId, WorkId, adjacent_expressions,
};
use words_to_data::link::Link;
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::query::DEFAULT_LIMIT;
use words_to_data::storage::LinkReader;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// Section 27(a)(2) of the Food and Nutrition Act of 2008 (7 U.S.C.
/// 2036(a)(2)), amended by striking "section 3(u)(4)" and inserting
/// "section 3(u)(3)". The Code prints "section 2012(u)(4) of this title", so
/// the quoted words show in neither change under the address.
const STRIKES_SECTION_3_U_4: &str = "a9fd405d5415";
const PARAGRAPH_2036_A_2: &str = "uscode/title_7/chapter_51/section_2036/subsection_a/paragraph_2";

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

/// The bill and titles 7 and 26 at each committed release point, with the
/// renumberings the bill states recorded over every window, as `build-dataset`
/// records them, and the committed OLRC classification table stated as links,
/// as `add-classifications` states them.
fn built() -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    for date in RELEASE_POINTS {
        for title in ["usc07.xml", "usc26.xml"] {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/{title}"), date, None)
                .expect("the title should parse");
        }
    }
    let windows = adjacent_expressions(&dataset).expect("the windows should list");
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill is held as a document");
    dataset
        .record_redesignations_over(BILL_ID, &bill.root, &windows)
        .expect("the renumberings should record");
    for link in olrc_links(&dataset) {
        dataset.add_link(link).expect("the link should add");
    }
    dataset
}

/// The committed classification table, as `olrc.classified_from` links against
/// what the dataset holds.
fn olrc_links(dataset: &Dataset<words_to_data::storage::InMemoryStorage>) -> Vec<Link> {
    let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
        .expect("the committed table should read");
    let table = ClassificationTable::parse(&html).expect("the committed table should parse");
    let mut paths = SectionPaths::new();
    for date in RELEASE_POINTS {
        for work in ["uscode/title_7", "uscode/title_26"] {
            let expression = dataset
                .get_expression(&ExpressionId::new(WorkId::new(work), date))
                .expect("storage should answer")
                .expect("the title is held");
            paths.add_work(&expression.root);
        }
    }
    let scope = dataset.scope().expect("the scope should derive");
    classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links
}

/// One run of the binary.
fn run(command: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg(command)
        .args(args)
        .output()
        .expect("the binary should run")
}

/// The committed corpus, saved as a database before anything links an
/// amendment, once for every case. No case writes over it.
fn unlinked() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = PathBuf::from(format!(
            "{}/residue_built.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        ));
        let _ = std::fs::remove_file(&path);
        built()
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// The committed corpus, saved as a database and linked by `link-by-evidence`,
/// once for every case. No case writes over it.
fn linked() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = PathBuf::from(format!(
            "{}/residue_linked.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        ));
        std::fs::copy(unlinked(), &path).expect("the fixture should copy");
        let output = run("link-by-evidence", &[path.to_str().expect("a UTF-8 path")]);
        assert!(
            output.status.success(),
            "link-by-evidence should exit zero, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        path
    })
}

/// The rows `residue --json` lists for the bill.
fn residue_rows(dataset: &Path) -> Vec<serde_json::Value> {
    let output = run(
        "residue",
        &[
            dataset.to_str().expect("a UTF-8 path"),
            "--bill",
            BILL_ID,
            "--json",
        ],
    );
    assert!(
        output.status.success(),
        "residue should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    json["rows"].as_array().expect("a rows array").clone()
}

/// The row for one amendment, by the start of its id, if it is listed.
fn row_of<'a>(rows: &'a [serde_json::Value], id_start: &str) -> Option<&'a serde_json::Value> {
    let found: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| {
            row["amendment_id"]
                .as_str()
                .is_some_and(|id| id.starts_with(id_start))
        })
        .collect();
    assert!(found.len() <= 1, "one row at most for {id_start}");
    found.first().copied()
}

#[test]
fn should_list_an_amendment_with_its_stage_reason_window_and_changes_when_no_link_names_it() {
    let rows = residue_rows(linked());

    let row = row_of(&rows, STRIKES_SECTION_3_U_4).expect("the amendment is listed");
    assert_eq!(row["stage"], "resolve");
    assert!(
        row["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("show in none")),
        "the reason says the quoted words were not found: {}",
        row["reason"]
    );
    assert_eq!(row["address"]["section"], "/us/usc/t7/s2036");
    assert_eq!(
        row["from"],
        serde_json::json!({"work": "uscode/title_7", "at": "2025-07-18"})
    );
    assert_eq!(
        row["to"],
        serde_json::json!({"work": "uscode/title_7", "at": "2025-07-30"})
    );
    assert_eq!(
        row["changes"],
        serde_json::json!([
            format!("{PARAGRAPH_2036_A_2}/subparagraph_C"),
            format!("{PARAGRAPH_2036_A_2}/subparagraph_E"),
        ])
    );
}

/// A copy of the linked fixture that one case may write into.
fn writable_copy(case: &str) -> PathBuf {
    let path = PathBuf::from(format!("{}/{case}.sqlite", env!("CARGO_TARGET_TMPDIR")));
    std::fs::copy(linked(), &path).expect("the fixture should copy");
    path
}

/// An amendment's full id, from the listing, by the start of it.
fn full_id(id_start: &str) -> String {
    row_of(&residue_rows(linked()), id_start)
        .and_then(|row| row["amendment_id"].as_str())
        .expect("the amendment is listed")
        .to_string()
}

#[test]
fn should_leave_the_list_when_an_agent_records_a_link_through_the_door() {
    let dataset = writable_copy("residue_after_the_door");
    assert!(
        row_of(&residue_rows(&dataset), STRIKES_SECTION_3_U_4).is_some(),
        "the amendment is listed before anyone links it"
    );

    let output = run(
        "link-amendment",
        &[
            dataset.to_str().expect("a UTF-8 path"),
            "--bill",
            BILL_ID,
            "--amendment",
            &full_id(STRIKES_SECTION_3_U_4),
            "--from",
            "uscode/title_7@2025-07-18",
            "--to",
            "uscode/title_7@2025-07-30",
            "--path",
            &format!("{PARAGRAPH_2036_A_2}/subparagraph_C"),
            "--source",
            "agent:claude",
            "--method",
            "resolve-residue@1",
            "--reason",
            "The Code prints section 3(u)(4) of the Act as section 2012(u)(4) of this title, \
             and subparagraph (C) is where that reference changed.",
        ],
    );
    assert!(
        output.status.success(),
        "link-amendment should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        row_of(&residue_rows(&dataset), STRIKES_SECTION_3_U_4).is_none(),
        "one link names the amendment, so it is no longer listed"
    );
}

/// Section 30001 of the law: "Section 1017(a)(2)(A)(iii) of the Consumer
/// Financial Protection Act of 2010 (12 U.S.C. 5497(a)(2)(A)(iii)) is amended
/// ...". The corpus holds no title 12, so the matcher stops at the window and
/// calls it work, and no change the dataset holds can be its change.
const AMENDS_TITLE_12: &str = "de4113b510a4";

#[test]
fn should_leave_the_work_and_show_as_reviewed_when_an_agent_concludes_an_amendment_has_no_link() {
    let dataset = writable_copy("residue_after_no_link");
    let before = residue_rows(&dataset);
    let row = row_of(&before, AMENDS_TITLE_12).expect("the amendment is listed");
    assert_eq!(row["category"], "work", "it is work before anyone looks");

    let reason = "The amendment changes 12 U.S.C. 5497, and the dataset holds no title 12.";
    let output = run(
        "link-amendment",
        &[
            dataset.to_str().expect("a UTF-8 path"),
            "--bill",
            BILL_ID,
            "--amendment",
            &full_id(AMENDS_TITLE_12),
            "--no-link",
            "not_held",
            "--source",
            "agent:claude",
            "--method",
            "resolve-residue@1",
            "--reason",
            reason,
        ],
    );
    assert!(
        output.status.success(),
        "link-amendment should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = residue_rows(&dataset);
    let row = row_of(&after, AMENDS_TITLE_12).expect("it is still listed, as reviewed");
    assert_eq!(row["category"], "reviewed_no_link");
    assert_eq!(row["no_link"]["category"], "not_held");
    assert_eq!(row["no_link"]["reviewer"], "agent:claude");
    assert_eq!(row["no_link"]["reasoning"], reason);
    let work =
        |rows: &[serde_json::Value]| rows.iter().filter(|row| row["category"] == "work").count();
    assert_eq!(work(&after), work(&before) - 1, "one less row of work");

    let printed = run(
        "residue",
        &[dataset.to_str().expect("a UTF-8 path"), "--bill", BILL_ID],
    );
    let stdout = String::from_utf8_lossy(&printed.stdout);
    assert!(
        stdout.contains("reviewed: no link"),
        "a person sees the reviewed count: {stdout}"
    );
}

/// A "no link" conclusion is a record like any review, so a later reviewer
/// overrides it by publishing over it, and the newest record wins
/// (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
#[test]
fn should_return_to_the_work_when_a_later_reviewer_refutes_the_no_link_conclusion() {
    let dataset = writable_copy("residue_after_refuted_no_link");
    let path = dataset.to_str().expect("a UTF-8 path");
    let concluded = run(
        "link-amendment",
        &[
            path,
            "--bill",
            BILL_ID,
            "--amendment",
            &full_id(AMENDS_TITLE_12),
            "--no-link",
            "no_change",
            "--source",
            "agent:claude",
            "--method",
            "resolve-residue@1",
            "--reason",
            "An agent that did not look far enough.",
        ],
    );
    assert!(concluded.status.success(), "the conclusion should record");
    let record = Dataset::open_sqlite(&dataset)
        .expect("the dataset should open")
        .links_by_kind("review.no_link")
        .expect("the records should read")
        .pop()
        .expect("one no-link record");

    let refuted = run(
        "settle",
        &[
            path,
            "--link",
            &record.id(),
            "--verdict",
            "refuted",
            "--reviewer",
            "human:jesse",
            "--reason",
            "The amendment does change the text; it is in title 12, which is not held.",
        ],
    );
    assert!(
        refuted.status.success(),
        "settle should refute the conclusion, stderr: {}",
        String::from_utf8_lossy(&refuted.stderr)
    );

    let rows = residue_rows(&dataset);
    let row = row_of(&rows, AMENDS_TITLE_12).expect("the amendment is listed");
    assert_eq!(
        row["category"], "work",
        "the refuted conclusion no longer stands"
    );
}

#[test]
fn should_report_an_amendment_as_quiet_and_not_as_work_when_nothing_under_its_address_changed() {
    // Section 70116(b)(1) of the law: "Section 25B(a) is amended by striking
    // "$2,000" and inserting "$2,100"." The Code holds § 25B(a), and it reads
    // the same at every release point the corpus holds after the law. A corpus
    // that does not span the date an amendment takes effect is the ordinary
    // state of a growing dataset (#211), so there is nothing to resolve.
    let rows = residue_rows(linked());

    let row = row_of(&rows, "3f2a48cdb3bc").expect("the amendment is listed");
    assert_eq!(row["category"], "quiet");
    assert_eq!(row["stage"], "window");
    assert_eq!(row["address"]["section"], "/us/usc/t26/s25B");

    // An amendment with changes under its address is work.
    let work = row_of(&rows, STRIKES_SECTION_3_U_4).expect("the amendment is listed");
    assert_eq!(work["category"], "work");
}

#[test]
fn should_not_call_an_amendment_quiet_when_the_code_held_has_nothing_at_its_address() {
    // Section 70118(a) of the law: "Section 11026(a) of Public Law 115-97 is
    // amended ...". Before #259 the markup reader read it as § 11026 of title
    // 26, which title 26 does not have, and this case checked that the window
    // stage did not call it quiet. It now stops at the address, because
    // § 11026 is a section of another law, and no other amendment of the
    // committed corpus has an address the held Code lacks. The rule stays: an
    // amendment with nothing to act on is never quiet.
    let rows = residue_rows(linked());

    let row = row_of(&rows, "5219c7e9a020").expect("the amendment is listed");
    assert_ne!(row["category"], "quiet");
}

#[test]
fn should_show_the_olrc_classification_of_the_amendments_section_of_the_law_when_one_is_stored() {
    // The amendment sits at section 10101(b)(3) of the law, and the table
    // classifies 10101(b)(3) to 7 U.S.C. 2036, as an amendment of the section.
    let rows = residue_rows(linked());

    let row = row_of(&rows, STRIKES_SECTION_3_U_4).expect("the amendment is listed");
    assert_eq!(row["law_section"], "10101(b)(3)");
    assert_eq!(
        row["olrc"],
        serde_json::json!([{
            "law_section": "10101(b)(3)",
            "code_section": "uscode/title_7/chapter_51/section_2036",
            "descriptions": [""],
        }])
    );
}

#[test]
fn should_report_an_amendment_to_a_note_as_not_held_and_not_as_a_miss() {
    // Section 70118(a) of the law amends section 11026 of Public Law 115-97.
    // The table classifies 70118(a)-(c) to 26 U.S.C. 112 as `nt`: a note
    // under § 112. The dataset holds no notes, so no change it holds can be
    // this amendment's, and an agent has nothing to look for.
    let rows = residue_rows(linked());

    let row = row_of(&rows, "5219c7e9a020").expect("the amendment is listed");
    assert_eq!(row["category"], "not_held");
    assert_eq!(
        row["olrc"],
        serde_json::json!([{
            "law_section": "70118(a)-(c)",
            "code_section": "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_III/section_112",
            "descriptions": ["nt"],
        }])
    );
    assert_eq!(
        row["not_held"],
        "the OLRC classifies this section of the law only as a note or as the heading before \
         a section, and the dataset holds neither"
    );
}

#[test]
fn should_report_an_amendment_to_a_table_of_sections_as_not_held_and_not_as_a_miss() {
    // Section 70201(g) of the law: "The table of sections for part VII of
    // subchapter B of chapter 1 is amended by ...". A table of sections is an
    // index of the law, and the dataset does not hold it as a provision.
    let rows = residue_rows(linked());

    let row = row_of(&rows, "4010e01c92b0").expect("the amendment is listed");
    assert_eq!(row["stage"], "address");
    assert_eq!(row["category"], "not_held");
    assert_eq!(
        row["not_held"],
        "a table of sections, which the dataset does not hold as a provision"
    );
}

#[test]
fn should_say_the_batch_has_not_written_its_link_when_the_method_links_an_amendment_no_link_names()
{
    // Section 10102(c) of the law renumbers paragraph (7) of 7 U.S.C. 2015(o)
    // as (8) and inserts a new (7). The evidence method links it, and in this
    // dataset `link-by-evidence` has not run, so no link names it yet. That is
    // not work for an agent: the batch writes it.
    let rows = residue_rows(unlinked());

    let row = row_of(&rows, "d624331f459d").expect("the amendment is listed");
    assert_eq!(row["category"], "unwritten");
    assert!(row["stage"].is_null(), "the method did not stop");
    assert_eq!(
        row["reason"],
        "the evidence method links it, and no link is written: run link-by-evidence"
    );
    assert_eq!(
        row["from"],
        serde_json::json!({"work": "uscode/title_7", "at": "2025-07-18"})
    );
    assert!(
        row["changes"]
            .as_array()
            .expect("a list of changes")
            .contains(&serde_json::json!(
                "uscode/title_7/chapter_51/section_2015/subsection_o/paragraph_8"
            )),
        "the changes it would be linked to: {}",
        row["changes"]
    );
}

#[test]
fn should_store_nothing_when_it_lists_the_residue() {
    let dataset = writable_copy("residue_stores_nothing");
    let links = || {
        Dataset::open_sqlite(&dataset)
            .expect("the dataset should open")
            .count_links_by_kind()
            .expect("the links should count")
    };
    let before = links();

    residue_rows(&dataset);
    let output = run(
        "residue",
        &[dataset.to_str().expect("a UTF-8 path"), "--bill", BILL_ID],
    );
    assert!(output.status.success());

    assert_eq!(links(), before, "no link is written or removed");
}

#[test]
fn should_print_at_most_a_screenful_and_say_how_many_rows_it_left_out_when_the_output_is_for_a_person()
 {
    let total = residue_rows(linked()).len();
    assert!(
        total > DEFAULT_LIMIT,
        "the corpus gives more than a screenful"
    );

    let output = run(
        "residue",
        &[linked().to_str().expect("a UTF-8 path"), "--bill", BILL_ID],
    );
    assert!(
        output.status.success(),
        "residue should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Each row leads with the start of the amendment's id.
    let shown = residue_rows(linked())
        .iter()
        .filter(|row| {
            let id = row["amendment_id"].as_str().expect("an id");
            stdout.contains(&format!("  {}  ", &id[..12]))
        })
        .count();
    assert_eq!(shown, DEFAULT_LIMIT, "a screenful of rows: {stdout}");
    assert!(
        stdout.contains(&format!(
            "{} more row(s) not shown; pass --json for all of them",
            total - DEFAULT_LIMIT
        )),
        "the output says what it left out: {stdout}"
    );
}

/// "Section 359l(a) of the Agricultural Adjustment Act of 1938 (7 U.S.C.
/// 1359ll(a)) is amended by striking "2023" and inserting "2031"."
/// `link-by-evidence` names it with one link, to § 1359ll(a).
const EXTENDS_1359LL_A: &str = "0c620652089127d0";

/// Every `amended_by` link that names one amendment, by the start of its id.
fn links_naming(dataset: &Path, id_start: &str) -> Vec<Link> {
    Dataset::open_sqlite(dataset)
        .expect("the dataset should open")
        .links_for_object_prefix(&format!("legislature.amendment:{BILL_ID}:{id_start}"))
        .expect("the links should read")
        .into_iter()
        .filter(|link| link.kind.0 == "legislature.amended_by")
        .collect()
}

/// Record one review of a link through `settle`.
fn settle(dataset: &Path, link_id: &str, verdict: &str, reviewer: &str, reason: &str) {
    let output = run(
        "settle",
        &[
            dataset.to_str().expect("a UTF-8 path"),
            "--link",
            link_id,
            "--verdict",
            verdict,
            "--reviewer",
            reviewer,
            "--reason",
            reason,
        ],
    );
    assert!(
        output.status.success(),
        "settle should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A refutation says that a link is wrong. It does not say where the
/// amendment's change is. So an amendment whose every link is refuted is work
/// again (#268).
#[test]
fn should_list_an_amendment_as_work_and_name_the_refuted_link_when_its_only_link_is_refuted() {
    let dataset = writable_copy("residue_after_refuted_link");
    assert!(
        row_of(&residue_rows(&dataset), EXTENDS_1359LL_A).is_none(),
        "a link names the amendment, so it is not listed"
    );
    let links = links_naming(&dataset, EXTENDS_1359LL_A);
    assert_eq!(links.len(), 1, "one link names the amendment");
    let link_id = links[0].id();

    let reason = "The change at subsection (a) is not this amendment's change.";
    settle(&dataset, &link_id, "refuted", "agent:claude", reason);

    let rows = residue_rows(&dataset);
    let row = row_of(&rows, EXTENDS_1359LL_A).expect("the amendment is listed again");
    assert_eq!(row["category"], "work");
    assert_eq!(row["reason"], "its every link was refuted");
    assert_eq!(row["refuted"].as_array().map(Vec::len), Some(1));
    assert_eq!(row["refuted"][0]["link_id"], link_id);
    assert_eq!(row["refuted"][0]["reviewer"], "agent:claude");
    assert_eq!(row["refuted"][0]["reasoning"], reason);

    let printed = run(
        "residue",
        &[dataset.to_str().expect("a UTF-8 path"), "--bill", BILL_ID],
    );
    let stdout = String::from_utf8_lossy(&printed.stdout);
    assert!(
        stdout.contains(&format!("refuted link {} by agent:claude", &link_id[..12]))
            && stdout.contains(reason),
        "a person sees the refuted link, its reviewer and the reason: {stdout}"
    );
    assert!(
        stdout.contains("   1  review: its every link was refuted"),
        "the work count says a review put it there, not the method: {stdout}"
    );
}

/// The newest review of a link wins, whoever wrote it
/// (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
#[test]
fn should_leave_the_list_when_a_newer_review_confirms_the_refuted_link() {
    let dataset = writable_copy("residue_after_confirmed_again");
    let links = links_naming(&dataset, EXTENDS_1359LL_A);
    assert_eq!(links.len(), 1, "one link names the amendment");
    let link_id = links[0].id();
    settle(
        &dataset,
        &link_id,
        "refuted",
        "agent:claude",
        "The change at subsection (a) is not this amendment's change.",
    );
    assert!(
        row_of(&residue_rows(&dataset), EXTENDS_1359LL_A).is_some(),
        "the amendment is listed while its only link is refuted"
    );

    settle(
        &dataset,
        &link_id,
        "confirmed",
        "human:jesse",
        "Subsection (a) is where \"2023\" became \"2031\".",
    );

    assert!(
        row_of(&residue_rows(&dataset), EXTENDS_1359LL_A).is_none(),
        "the newest review confirms the link, so the link stands"
    );
}

/// Section 10102(c) of the law renumbers paragraph (7) of 7 U.S.C. 2015(o) as
/// (8) and inserts a new (7). `link-by-evidence` names it with two links.
const RENUMBERS_2015_O_7: &str = "d624331f459d";

#[test]
fn should_not_list_an_amendment_when_one_of_its_links_is_refuted_and_another_stands() {
    let dataset = writable_copy("residue_after_one_of_two_refuted");
    let links = links_naming(&dataset, RENUMBERS_2015_O_7);
    assert_eq!(links.len(), 2, "two links name the amendment");

    settle(
        &dataset,
        &links[0].id(),
        "refuted",
        "agent:claude",
        "This path is not where the amendment acts.",
    );

    assert!(
        row_of(&residue_rows(&dataset), RENUMBERS_2015_O_7).is_none(),
        "the other link stands, so the amendment is linked"
    );
}

/// A conclusion that an amendment has no link settles it. A refutation only
/// says that one link is wrong, so the conclusion takes precedence.
#[test]
fn should_show_as_reviewed_and_not_as_work_when_its_links_are_refuted_and_a_no_link_conclusion_stands()
 {
    let dataset = writable_copy("residue_after_refuted_then_no_link");
    let links = links_naming(&dataset, EXTENDS_1359LL_A);
    assert_eq!(links.len(), 1, "one link names the amendment");
    let link_id = links[0].id();
    settle(
        &dataset,
        &link_id,
        "refuted",
        "agent:claude",
        "The change at subsection (a) is not this amendment's change.",
    );

    let output = run(
        "link-amendment",
        &[
            dataset.to_str().expect("a UTF-8 path"),
            "--bill",
            BILL_ID,
            "--amendment",
            &full_id_in(&dataset, EXTENDS_1359LL_A),
            "--no-link",
            "no_change",
            "--source",
            "agent:claude",
            "--method",
            "resolve-residue@1",
            "--reason",
            "The text at the address did not change in any window the dataset holds.",
        ],
    );
    assert!(
        output.status.success(),
        "link-amendment should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let rows = residue_rows(&dataset);
    let row = row_of(&rows, EXTENDS_1359LL_A).expect("the amendment is listed, as reviewed");
    assert_eq!(row["category"], "reviewed_no_link");
    assert_eq!(row["no_link"]["category"], "no_change");
    assert_eq!(
        row["refuted"][0]["link_id"], link_id,
        "the row still names the refuted link"
    );
}

/// An amendment's full id, from the listing of one dataset, by the start of it.
fn full_id_in(dataset: &Path, id_start: &str) -> String {
    row_of(&residue_rows(dataset), id_start)
        .and_then(|row| row["amendment_id"].as_str())
        .expect("the amendment is listed")
        .to_string()
}
