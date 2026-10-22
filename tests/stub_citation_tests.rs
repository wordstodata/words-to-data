//! A citation whose section is now a stub says so (#310).
//!
//! Of the 55,154 sections in title-level printings, 4,171 are stubs: the
//! heading says the section was Repealed, Renumbered, Transferred or Omitted,
//! and there is no law under it. A citation that resolves to one looked like a
//! good link. *United Stationers* (N.D. Ill. 1997) cites `26 U.S.C. § 44F`, and
//! the held printings of title 26 print § 44F as `[§ 44F. Renumbered § 30]`.
//!
//! Every record here is real: title 26 from the committed corpus, and the
//! committed CourtListener records in `tests/test_data/courtlistener`.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::citation::resolve::{SectionPaths, resolve, stub_heading};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::judicial::cites::citations_naming;
use words_to_data::judicial::reliance;
use words_to_data::query::PathMatch;
use words_to_data::storage::InMemoryStorage;

const CACHE: &str = "tests/test_data/courtlistener";

/// *United Stationers, Inc. v. United States* (N.D. Ill. 1997): its opinion
/// and its cluster share the id.
const UNITED_STATIONERS: u64 = 1527901;

const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const RELEASE: &str = "2025-07-30";

/// Section 44F as the 2025-07-30 release point publishes it.
const SECTION_44F: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_44F";

/// Section 174 as the 2025-07-30 release point publishes it.
const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

fn title_26() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Title 26".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_26, RELEASE, None)
        .expect("title 26 should parse");
    dataset
}

#[test]
fn should_give_the_stub_heading_when_a_section_is_renumbered() {
    let dataset = title_26();
    let title = dataset
        .get_expression(&ExpressionId::new(WorkId::new("uscode/title_26"), RELEASE))
        .expect("storage should answer")
        .expect("title 26 should be held");

    let section_44f = title.root.find(SECTION_44F).expect("§ 44F is held");
    let section_174 = title.root.find(SECTION_174).expect("§ 174 is held");

    assert_eq!(
        stub_heading(section_44f).as_deref(),
        // The Code sets a narrow no-break space after the section sign.
        Some("[Renumbered \u{a7}\u{202f}30]")
    );
    assert_eq!(stub_heading(section_174), None, "§ 174 is law, not a stub");
}

/// Title 26, *United Stationers*, and the links its text makes, the way
/// `add-opinions` writes them.
fn title_26_and_united_stationers() -> Dataset<InMemoryStorage> {
    let mut dataset = title_26();
    let title = dataset
        .get_expression(&ExpressionId::new(WorkId::new("uscode/title_26"), RELEASE))
        .expect("storage should answer")
        .expect("title 26 should be held");
    let mut index = SectionPaths::new();
    index.add_work(&title.root);

    let read = |name: String| {
        std::fs::read_to_string(format!("{CACHE}/{name}"))
            .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
    };
    let opinion = OpinionRecord::from_json(&read(format!("opinion_{UNITED_STATIONERS}.json")))
        .expect("the opinion should read");
    let cluster = ClusterRecord::from_json(&read(format!("cluster_{UNITED_STATIONERS}.json")))
        .expect("the cluster should read");
    let case = case_expression(&cluster, &[opinion]).expect("the case should build");
    let writing = case.writings[0].path.clone();
    let text = case
        .expression
        .root
        .find(&writing)
        .and_then(|node| node.data.content.clone())
        .expect("a writing has text");
    dataset
        .add_expression(case.expression)
        .expect("the case should store");

    let scope = dataset.scope().expect("the scope should derive");
    let citing = Opinion::held(writing, UNITED_STATIONERS.to_string(), cluster.display());
    for citation in usc::find_with_report(&text).0 {
        let cited = resolve(&citation, &scope, &index);
        for link in cites_links(&citing, &citation, &cited) {
            dataset.add_link(link).expect("a link should store");
        }
    }
    dataset
}

#[test]
fn should_flag_a_citation_with_the_stub_heading_when_cases_citing_reads_a_stub() {
    let dataset = title_26_and_united_stationers();

    let stub = reliance::cases_citing(&dataset, SECTION_44F).expect("the query should run");
    let law = reliance::cases_citing(&dataset, SECTION_174).expect("the query should run");

    assert_eq!(stub.len(), 1, "United Stationers cites § 44F, got {stub:?}");
    assert_eq!(
        stub[0].cites[0].stub_heading.as_deref(),
        Some("[Renumbered \u{a7}\u{202f}30]"),
        "the citation of § 44F lands on a stub"
    );
    assert_eq!(law.len(), 1, "United Stationers cites § 174, got {law:?}");
    assert_eq!(
        law[0].cites[0].stub_heading, None,
        "a citation of a section of law is not flagged"
    );
}

#[test]
fn should_flag_a_citation_link_with_the_stub_heading_when_a_path_reader_reads_a_stub() {
    let dataset = title_26_and_united_stationers();

    let stub =
        citations_naming(&dataset, SECTION_44F, PathMatch::Subtree).expect("the query should run");
    let law =
        citations_naming(&dataset, SECTION_174, PathMatch::Subtree).expect("the query should run");

    assert_eq!(stub.len(), 1, "one link names § 44F, got {stub:?}");
    assert_eq!(
        stub[0].stub_heading.as_deref(),
        Some("[Renumbered \u{a7}\u{202f}30]")
    );
    assert_eq!(law.len(), 1, "one link names § 174, got {law:?}");
    assert_eq!(law[0].stub_heading, None);
}

// --- The commands -----------------------------------------------------------

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// What `add-opinions` printed when it built the fixture, and where the
/// fixture is: title 26 and *United Stationers*, added by the command offline.
fn fixture() -> &'static (String, String) {
    static FIXTURE: OnceLock<(String, String)> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/stub_citations.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        title_26()
            .save_to_sqlite(&path)
            .expect("the fixture should save");

        let added = run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--opinions",
            &UNITED_STATIONERS.to_string(),
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the opinion, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        (path, String::from_utf8_lossy(&added.stdout).to_string())
    })
}

fn stdout_of(args: &[&str]) -> String {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn should_print_the_stub_heading_when_path_shows_a_citation_of_a_stub() {
    let (dataset, _) = fixture();

    let stub = stdout_of(&["path", dataset, SECTION_44F]);
    let law = stdout_of(&["path", dataset, SECTION_174]);

    assert!(
        stub.contains("the cited section is now a stub: [Renumbered \u{a7}\u{202f}30]"),
        "got:\n{stub}"
    );
    assert!(
        law.contains("cited by judicial/case_1527901/opinion_1527901"),
        "got:\n{law}"
    );
    assert!(!law.contains("stub"), "§ 174 is not flagged, got:\n{law}");
}

#[test]
fn should_print_the_stub_heading_when_cases_citing_answers_about_a_stub() {
    let (dataset, _) = fixture();

    let stub = stdout_of(&["cases-citing", dataset, "--cites", "26 U.S.C. § 44F"]);
    let law = stdout_of(&["cases-citing", dataset, "--path", SECTION_174]);

    assert!(
        stub.contains("the cited section is now a stub: [Renumbered \u{a7}\u{202f}30]"),
        "got:\n{stub}"
    );
    assert!(law.contains("1 opinion(s) cite it"), "got:\n{law}");
    assert!(!law.contains("stub"), "§ 174 is not flagged, got:\n{law}");
}

#[test]
fn should_count_the_citations_that_land_on_a_stub_when_add_opinions_links_them() {
    let (_, printed) = fixture();

    assert!(
        printed.contains("1 of them land on a stub"),
        "United Stationers cites § 44F once, got:\n{printed}"
    );
}

/// The `stub_heading` of every object in `value` that carries one, wherever it
/// is: each command nests its citations in its own way.
fn stub_fields(value: &serde_json::Value, into: &mut Vec<serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(stub) = map.get("stub_heading") {
                into.push(stub.clone());
            }
            map.values().for_each(|inner| stub_fields(inner, into));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|inner| stub_fields(inner, into)),
        _ => {}
    }
}

#[test]
fn should_carry_the_stub_heading_in_json_when_a_reader_reports_a_citation() {
    let (dataset, _) = fixture();
    let heading = serde_json::json!("[Renumbered \u{a7}\u{202f}30]");

    for args in [
        vec!["path", dataset, SECTION_44F, "--json"],
        vec!["annotations", dataset, "--path", SECTION_44F, "--json"],
        vec!["cases-citing", dataset, "--path", SECTION_44F, "--json"],
    ] {
        let printed = stdout_of(&args);
        let answer: serde_json::Value =
            serde_json::from_str(&printed).unwrap_or_else(|error| panic!("{error}: {printed}"));
        let mut found = Vec::new();
        stub_fields(&answer, &mut found);
        assert_eq!(
            found,
            std::slice::from_ref(&heading),
            "{args:?} got:\n{printed}"
        );
    }

    let printed = stdout_of(&["path", dataset, SECTION_174, "--json"]);
    let answer: serde_json::Value = serde_json::from_str(&printed).expect("JSON");
    let mut found = Vec::new();
    stub_fields(&answer, &mut found);
    assert_eq!(
        found,
        [serde_json::Value::Null],
        "the field is there, and empty, for a section of law"
    );
}
