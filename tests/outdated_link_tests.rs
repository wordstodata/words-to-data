//! Outdated links: a link that the current version of a batch method no
//! longer makes (#185).
//!
//! A link's identity is its subject, kind and object
//! (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`), so a
//! newer run re-stamps every link it makes again with its own version. A link
//! that only the older version made keeps the older version, because evidence
//! is never deleted (`docs/adr/0005`). Among one batch method's links for one
//! bill in one window, a link below the newest version there is outdated. This
//! is derived every time and never stored
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! Every case reads the committed corpus: title 26 before and after `119-hr-1`
//! reached the Code, and the committed public law. One case also reads a second
//! committed law, Pub. L. 119-22.
//!
//! **The test seam.** A method's version is chosen by a person and lives in
//! the code, so a test cannot run an older build of the matcher. A case stands
//! for an older run by storing the matcher's own links with their method set to
//! the older version, and for a newer run that no longer makes a link by
//! storing the matcher's links as they come, less that one. Every link is one
//! the matcher made from the committed corpus; only its version is set.

use std::process::Command;
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, Format, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{evidence_method, match_by_evidence};
use words_to_data::legislature::outdated::outdated_links;
use words_to_data::legislature::redesignation::reading_method;
use words_to_data::link::{Link, LinkKind, amendment_reference_parts};
use words_to_data::method::Method;
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The committed public law.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";

/// The section `119-hr-1` inserts after § 174, which the matcher links.
const SECTION_174A: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A";

/// A committed law, as the Congress client would hand it over.
fn committed_download(dir: &str, bill_id: &str) -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{dir}/{name}"))
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

/// Title 26 at both release points and the public law, with no link.
fn corpus() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (file, date) in [
        (TITLE_26_BEFORE, "2025-07-18"),
        (TITLE_26_AFTER, "2025-07-30"),
    ] {
        dataset
            .add_uslm_xml(file, date, None)
            .expect("title 26 should load");
    }
    dataset
        .load_bill_download(&committed_download(BILL_DIR, BILL_ID))
        .expect("the bill should load");
    dataset
}

/// Every link the matcher makes from the committed corpus, once for every case.
fn matcher_links() -> &'static [Link] {
    static LINKS: OnceLock<Vec<Link>> = OnceLock::new();
    LINKS.get_or_init(|| {
        match_by_evidence(&corpus())
            .expect("the matcher should run")
            .matches
            .iter()
            .flat_map(|amendment| amendment.links())
            .collect()
    })
}

/// The matcher's link to a path.
fn matcher_link_to(path: &str) -> &'static Link {
    matcher_links()
        .iter()
        .find(|link| link.subject.path() == Some(path))
        .unwrap_or_else(|| panic!("the matcher should link {path}"))
}

/// The same link, as a run of the method at `version` would store it.
fn at_version(link: &Link, version: u32) -> Link {
    let mut link = link.clone();
    link.provenance.method = Some(Method::new(evidence_method().name, version));
    link
}

/// Store each link of a run.
fn store(dataset: &mut Dataset<InMemoryStorage>, links: impl IntoIterator<Item = Link>) {
    for link in links {
        dataset.add_link(link).expect("the link should store");
    }
}

/// The link the newer run no longer makes: `119-hr-1`'s new § 174A.
fn dropped() -> &'static Link {
    matcher_link_to(SECTION_174A)
}

/// The corpus after `119-hr-1` was linked by the method's previous version,
/// and then re-run at its current version, which made every link again but
/// [`dropped`].
fn re_run() -> Dataset<InMemoryStorage> {
    let mut dataset = corpus();
    let previous = evidence_method().version - 1;
    store(
        &mut dataset,
        matcher_links()
            .iter()
            .map(|link| at_version(link, previous)),
    );
    store(
        &mut dataset,
        matcher_links()
            .iter()
            .filter(|link| link.id() != dropped().id())
            .cloned(),
    );
    dataset
}

/// [`re_run`], saved once as a compact dataset that the binary reads and no
/// case writes over.
fn saved_re_run() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/outdated_re_run.json", env!("CARGO_TARGET_TMPDIR"));
        re_run()
            .save(&path, Format::Compact)
            .expect("the dataset should save");
        path
    })
}

#[test]
fn should_mark_only_the_link_the_newer_version_did_not_make_again_when_a_bill_is_re_run() {
    let dataset = re_run();

    let outdated = outdated_links(&dataset, Some(BILL_ID)).expect("the rule should read");

    let ids: Vec<&str> = outdated.iter().map(|row| row.link_id.as_str()).collect();
    assert_eq!(ids, vec![dropped().id().as_str()]);
    let row = &outdated[0];
    let current = evidence_method().version;
    assert_eq!(
        row.made_by,
        Method::new(evidence_method().name, current - 1)
    );
    assert_eq!(row.remade_by, evidence_method());
    assert_eq!(row.path, SECTION_174A);
    assert_eq!(row.bill_id, BILL_ID);
    assert_eq!(row.window.from_date, "2025-07-18");
    assert_eq!(row.window.to_date, "2025-07-30");
}

#[test]
fn should_list_the_outdated_link_as_review_work_when_residue_is_asked_for_the_bill() {
    let output = run("residue", &[saved_re_run(), "--bill", BILL_ID, "--json"]);
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    let rows = json["outdated"].as_array().expect("an outdated array");
    assert_eq!(rows.len(), 1, "one link is outdated: {rows:?}");
    assert_eq!(rows[0]["link_id"], dropped().id());
    assert_eq!(rows[0]["path"], SECTION_174A);

    let output = run("residue", &[saved_re_run(), "--bill", BILL_ID]);
    let printed = String::from_utf8_lossy(&output.stdout);
    let section = printed
        .split("Links the current method no longer makes")
        .nth(1)
        .unwrap_or_else(|| panic!("residue should print the section: {printed}"));
    assert!(section.contains(&dropped().id()[..12]), "{section}");
    assert!(section.contains(SECTION_174A), "{section}");
    assert!(
        section.contains("@3") && section.contains("@4"),
        "{section}"
    );
}

/// Each `links` entry `annotations --json` prints for the bill.
fn annotation_links(dataset: &str) -> Vec<serde_json::Value> {
    let output = run("annotations", &[dataset, "--bill", BILL_ID, "--json"]);
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    json["annotations"]
        .as_array()
        .expect("an annotations array")
        .iter()
        .flat_map(|annotation| annotation["links"].as_array().cloned().unwrap_or_default())
        .collect()
}

#[test]
fn should_mark_the_outdated_link_and_no_other_when_annotations_lists_the_bill() {
    let links = annotation_links(saved_re_run());
    let short = &dropped().id()[..12];

    let (outdated, current): (Vec<_>, Vec<_>) = links
        .iter()
        .partition(|link| link["id"].as_str().is_some_and(|id| short.starts_with(id)));
    assert_eq!(outdated.len(), 1, "the dropped link is listed once");
    let marked = &outdated[0]["outdated"];
    assert_eq!(marked["made_by"]["version"], evidence_method().version - 1);
    assert_eq!(marked["remade_by"]["version"], evidence_method().version);
    assert!(
        current.iter().all(|link| link["outdated"].is_null()),
        "every link the newer version made again is current"
    );

    let output = run(
        "annotations",
        &[saved_re_run(), "--bill", BILL_ID, "--path", SECTION_174A],
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    let line = printed
        .lines()
        .find(|line| line.contains(SECTION_174A) && line.contains("link "))
        .unwrap_or_else(|| panic!("annotations should print the link: {printed}"));
    assert!(line.contains("outdated"), "{line}");
}

#[test]
fn should_say_the_link_is_outdated_and_which_version_re_made_the_window_when_settle_explains_it() {
    let output = run(
        "settle",
        &[saved_re_run(), "--link", &dropped().id()[..12], "--explain"],
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    let line = printed
        .lines()
        .find(|line| line.trim_start().starts_with("Outdated:"))
        .unwrap_or_else(|| panic!("settle --explain should say the link is outdated: {printed}"));
    assert!(
        line.contains("@3") && line.contains("@4"),
        "the line names both versions: {line}"
    );

    let current = matcher_links()
        .iter()
        .find(|link| link.id() != dropped().id())
        .expect("the matcher makes more than one link");
    let output = run(
        "settle",
        &[saved_re_run(), "--link", &current.id()[..12], "--explain"],
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        !printed.contains("Outdated:"),
        "a link the newer version made again is current: {printed}"
    );
}

#[test]
fn should_mark_the_outdated_link_when_path_reports_the_provision_it_names() {
    let output = run("path", &[saved_re_run(), SECTION_174A, "--exact", "--json"]);
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    let links: Vec<&serde_json::Value> = json["annotations"]
        .as_array()
        .expect("an annotations array")
        .iter()
        .flat_map(|annotation| annotation["links"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(links.len(), 1, "one link names § 174A: {links:?}");
    assert_eq!(
        links[0]["outdated"]["remade_by"]["version"],
        evidence_method().version
    );

    let output = run("path", &[saved_re_run(), SECTION_174A, "--exact"]);
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        printed
            .lines()
            .any(|line| line.contains("link ") && line.contains("outdated")),
        "path should mark the link: {printed}"
    );
}

/// The renumbering links `119-hr-1` states in the corpus, as the renumbering
/// step records them.
fn renumbering_links() -> Vec<Link> {
    let mut dataset = corpus();
    let windows = adjacent_expressions(&dataset).expect("the windows should list");
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill is held as a document");
    dataset
        .record_redesignations_over(BILL_ID, &bill, &windows)
        .expect("the renumberings should record");
    dataset
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read")
}

#[test]
fn should_mark_an_outdated_renumbering_link_when_path_follows_it() {
    let links = renumbering_links();
    let dropped = &links[0];
    let previous = reading_method().version - 1;
    let mut dataset = corpus();
    // Same seam as the matcher's: the older run made every renumbering link,
    // and the newer run made them all again but one.
    store(
        &mut dataset,
        links.iter().map(|link| {
            let mut link = link.clone();
            link.provenance.method = Some(Method::new(reading_method().name, previous));
            link
        }),
    );
    store(
        &mut dataset,
        links
            .iter()
            .filter(|link| link.id() != dropped.id())
            .cloned(),
    );
    let saved = output_for("renumbering_re_run");
    dataset
        .save(&saved, Format::Compact)
        .expect("the dataset should save");

    let from_path = dropped.subject.path().expect("a renumbering names a path");
    let output = run(
        "path",
        &[
            &saved,
            from_path,
            "--from",
            "uscode/title_26@2025-07-18",
            "--to",
            "uscode/title_26@2025-07-30",
            "--json",
        ],
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    let via: Vec<&serde_json::Value> = json["provisions"]
        .as_array()
        .expect("a provisions array")
        .iter()
        .flat_map(|provision| provision["via"].as_array().into_iter().flatten())
        .collect();
    let short = &dropped.id()[..12];
    let followed = via
        .iter()
        .find(|link| link["id"].as_str().is_some_and(|id| short.starts_with(id)))
        .unwrap_or_else(|| panic!("path should follow the renumbering: {via:?}"));
    assert_eq!(
        followed["outdated"]["remade_by"]["version"],
        reading_method().version
    );
}

/// The corpus with no link, saved once as a compact dataset that the binary
/// reads and no case writes over.
fn saved_corpus() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/outdated_corpus.json", env!("CARGO_TARGET_TMPDIR"));
        corpus()
            .save(&path, Format::Compact)
            .expect("the corpus should save");
        path
    })
}

/// A place for one case to write a dataset.
fn output_for(case: &str) -> String {
    let path = format!("{}/outdated_{case}.json", env!("CARGO_TARGET_TMPDIR"));
    // A result left by an earlier run must not pass for this run's result.
    let _ = std::fs::remove_file(&path);
    path
}

/// One run of the binary, which must succeed.
fn run(command: &str, args: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg(command)
        .args(args)
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "{command} should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Record, through `link-amendment`, the amendment and the path of one of the
/// matcher's links, as an agent following `method` records a link it found.
fn record_through_the_door(dataset: &str, output: &str, link: &Link, method: &str) {
    let reference = link.object.name();
    let (bill, amendment) =
        amendment_reference_parts(&reference).expect("the matcher's link names an amendment");
    run(
        "link-amendment",
        &[
            dataset,
            "--bill",
            bill,
            "--amendment",
            amendment,
            "--from",
            "uscode/title_26@2025-07-18",
            "--to",
            "uscode/title_26@2025-07-30",
            "--path",
            link.subject
                .path()
                .expect("the matcher's link names a path"),
            "--source",
            "agent:claude",
            "--method",
            method,
            "--reason",
            "The amendment's words are the words that changed at this path.",
            "--output",
            output,
        ],
    );
}

/// Pub. L. 119-22 (H.R. 42), a second committed law. The dataset names it
/// `119-hr-42`.
const SECOND_LAW_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/42";
const SECOND_LAW: &str = "119-hr-42";

/// The full id of the one amendment the second law states.
fn second_law_amendment(dataset: &Dataset<InMemoryStorage>) -> String {
    let bill = dataset
        .get_bill(SECOND_LAW)
        .expect("the dataset should answer")
        .expect("the second law is held");
    let ids: Vec<&String> = bill.amendments.keys().collect();
    assert_eq!(ids.len(), 1, "the second law states one amendment");
    ids[0].clone()
}

/// **A second seam, for bill grouping.** No other committed law makes a
/// batch link in a window where `119-hr-1` has links: every law the committed
/// release points hold besides it was already in the Code at the first of
/// them. So a newer run for the second law is stood for by its one amendment,
/// recorded through `link-amendment` under the evidence method's current
/// version, at a path that changed in `119-hr-1`'s window.
#[test]
fn should_not_mark_one_bills_links_outdated_when_a_newer_version_runs_for_another_bill_in_the_same_window()
 {
    let mut dataset = corpus();
    dataset
        .load_bill_download(&committed_download(SECOND_LAW_DIR, SECOND_LAW))
        .expect("the second law should load");
    let amendment = second_law_amendment(&dataset);
    let current = evidence_method().version;
    // `119-hr-1` was linked by the older version only.
    store(
        &mut dataset,
        matcher_links()
            .iter()
            .map(|link| at_version(link, current - 1)),
    );
    let before = output_for("two_bills_before");
    dataset
        .save(&before, Format::Compact)
        .expect("the dataset should save");

    let after = output_for("two_bills_after");
    run(
        "link-amendment",
        &[
            &before,
            "--bill",
            SECOND_LAW,
            "--amendment",
            &amendment,
            "--from",
            "uscode/title_26@2025-07-18",
            "--to",
            "uscode/title_26@2025-07-30",
            "--path",
            SECTION_174A,
            "--source",
            "rule:evidence_matching",
            "--method",
            &evidence_method().to_string(),
            "--reason",
            "Stands for a newer run of the evidence method for the second law.",
            "--output",
            &after,
        ],
    );

    let dataset = Dataset::load(&after, Format::Compact).expect("the dataset should load");
    let outdated = outdated_links(&dataset, None).expect("the rule should read");
    assert!(
        outdated.is_empty(),
        "a run for one bill says nothing about another bill's links: {} marked",
        outdated.len()
    );
}

#[test]
fn should_never_mark_an_agents_link_outdated_when_a_newer_version_of_its_method_links_the_same_window()
 {
    let first = output_for("agent_first");
    let second = output_for("agent_second");
    let links = matcher_links();

    record_through_the_door(saved_corpus(), &first, &links[0], "resolve-residue@1");
    record_through_the_door(&first, &second, &links[1], "resolve-residue@2");

    let dataset = Dataset::load(&second, Format::Compact).expect("the dataset should load");
    let outdated = outdated_links(&dataset, None).expect("the rule should read");
    assert!(
        outdated.is_empty(),
        "an agent covers only the items it chose, so none of its links is outdated: {outdated:?}"
    );
}
