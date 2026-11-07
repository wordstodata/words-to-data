//! A citation that names an edition of the Code (#449).
//!
//! The *Windsor* dissent of Justice Scalia cites "8 U.S.C. § 1105a(a) (1976
//! ed.)", the old rule on judicial review of a deportation order. The number
//! was used again: in the printings held, § 1105a(a) is "Employment
//! authorization for battered spouses". A citation that names an edition the
//! dataset does not hold is linked as before, and the link records the edition
//! (ADR 0014, decision 7: an older citation is resolved against the earliest
//! printing held and says so). `cases-citing` then says that the law the court
//! read is out of scope, and gives no window that calls the provision
//! unchanged.
//!
//! Every record here is real: titles 8 and 26 from the committed corpus, and
//! the committed CourtListener records in `tests/test_data/courtlistener`.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::citation::resolve::{SectionPaths, resolve};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::judicial::reliance;
use words_to_data::link::{Link, LinkKind};
use words_to_data::storage::LinkReader;

const CACHE: &str = "tests/test_data/courtlistener";

/// The dissent record of *United States v. Windsor* (2013): Roberts, Scalia
/// and Alito.
const WINDSOR_DISSENTS: u64 = 9515914;

/// § 1105a(a) of title 8 as the 2025 release points publish it: "Employment
/// authorization for battered spouses".
const SECTION_1105A_A: &str = "uscode/title_8/chapter_12/subchapter_I/section_1105a/subsection_a";

/// The text the dataset stores for one opinion record.
fn stored_text(id: u64) -> String {
    let json = std::fs::read_to_string(format!("{CACHE}/opinion_{id}.json"))
        .unwrap_or_else(|error| panic!("opinion {id} should be committed: {error}"));
    let record = OpinionRecord::from_json(&json).expect("the opinion record should read");
    let (text, _, _) = record.text().expect("the record carries text");
    text
}

#[test]
fn should_read_the_edition_when_a_citation_names_one() {
    let found = usc::find(&stored_text(WINDSOR_DISSENTS));

    let [cited] = found
        .iter()
        .filter(|citation| citation.title == "8" && citation.sections == ["1105a(a)"])
        .collect::<Vec<_>>()
        .try_into()
        .expect("the dissent cites § 1105a(a) once");
    let edition = cited
        .edition
        .as_ref()
        .expect("the citation names an edition");
    assert_eq!(edition.written, "1976 ed.");
    assert_eq!(edition.year, "1976");
}

const CLUSTER: u64 = 945737;

fn read_cached(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

/// Title 8 at two release points, the *Windsor* case from its two split
/// records, and the links the text of each writing makes, the way
/// `add-opinions` writes them. The combined record of the case is not
/// committed, so `add-opinions` cannot read the case offline.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/edition_citation.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 8 and Windsor".to_string(),
            ..Default::default()
        });
        for date in ["2025-07-18", "2025-07-30"] {
            let file = format!("tests/test_data/usc/{date}/usc08.xml");
            dataset
                .add_uslm_xml(&file, date, None)
                .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        }
        let title = dataset
            .get_expression(&ExpressionId::new(
                WorkId::new("uscode/title_8"),
                "2025-07-18",
            ))
            .expect("storage should answer")
            .expect("title 8 should be held");
        let mut index = SectionPaths::new();
        index.add_work(&title.root);

        let cluster = ClusterRecord::from_json(&read_cached(&format!("cluster_{CLUSTER}.json")))
            .expect("the cluster should read");
        let records = [9515913, WINDSOR_DISSENTS].map(|id| {
            OpinionRecord::from_json(&read_cached(&format!("opinion_{id}.json")))
                .expect("the opinion should read")
        });
        let case = case_expression(&cluster, &records).expect("the case should build");
        let decided = case.expression.id.at.clone();
        let writings: Vec<(String, String)> = case
            .writings
            .iter()
            .map(|writing| (writing.path.clone(), writing.text.clone()))
            .collect();
        dataset
            .add_expression(case.expression)
            .expect("the case should store");

        let scope = dataset.scope().expect("the scope should derive");
        for (writing, text) in writings {
            let citing = Opinion::held(writing, WINDSOR_DISSENTS.to_string(), cluster.display())
                .decided_on(decided.clone());
            for citation in usc::find_with_report(&text).0 {
                let cited = resolve(&citation, &scope, &index);
                for link in cites_links(&citing, &citation, &cited) {
                    dataset.add_link(link).expect("a link should store");
                }
            }
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}
fn citation_links() -> Vec<Link> {
    Dataset::open_sqlite(dataset_path())
        .expect("the fixture should open")
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
}

#[test]
fn should_record_the_edition_in_the_link_when_a_citation_names_one() {
    let links = citation_links();
    let [link] = links
        .iter()
        .filter(|link| link.object.path() == Some(SECTION_1105A_A))
        .collect::<Vec<_>>()
        .try_into()
        .expect("one passage cites § 1105a(a)");

    let value = &link
        .payload
        .as_ref()
        .expect("a citation has a payload")
        .value;
    assert_eq!(value["occurrences"][0]["edition"]["written"], "1976 ed.");
    assert_eq!(value["occurrences"][0]["edition"]["year"], "1976");
    let evidence = link
        .provenance
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.reasoning.as_deref())
        .expect("the link carries the words read");
    assert!(
        evidence.ends_with("(1976 ed.)"),
        "the evidence shows the edition, got {evidence:?}"
    );
    let method = link.provenance.method.as_ref().expect("a rule made it");
    assert_eq!(method.version, 7);
}

/// *Boeing Co. v. United States* (2003): "Compare 26 U. S. C. §§ 991-997 with
/// 26 U. S. C. §§ 921, 923 (1988 ed.)".
const BOEING: u64 = 122262;

#[test]
fn should_read_no_edition_when_the_brackets_follow_another_citation() {
    let found = usc::find(&stored_text(BOEING));

    let range = found
        .iter()
        .find(|citation| citation.sections == ["991-997"])
        .expect("Boeing cites §§ 991-997");
    let compared = found
        .iter()
        .find(|citation| citation.sections == ["921", "923"])
        .expect("Boeing cites §§ 921, 923");
    assert_eq!(
        range.edition, None,
        "the brackets belong to the next citation"
    );
    assert_eq!(
        compared
            .edition
            .as_ref()
            .map(|edition| edition.written.as_str()),
        Some("1988 ed.")
    );
}

#[test]
fn should_read_a_supplement_with_its_edition_when_a_citation_names_both() {
    let found = usc::find(&stored_text(9515913));

    let cited = found
        .iter()
        .find(|citation| citation.sections == ["1186a(b)(1)"])
        .expect("the opinion of the Court cites § 1186a(b)(1)");
    let edition = cited
        .edition
        .as_ref()
        .expect("the citation names an edition");
    assert_eq!(edition.written, "2006 ed. and Supp. V");
    assert_eq!(edition.year, "2006");
}

#[test]
fn should_give_no_window_when_the_edition_cited_is_not_held() {
    let dataset = Dataset::open_sqlite(dataset_path()).expect("the fixture should open");

    let cases = reliance::cases_citing(&dataset, SECTION_1105A_A).expect("the query should run");

    let [case] = cases.as_slice() else {
        panic!("one writing cites § 1105a(a), got {cases:?}");
    };
    let [cited] = case.cites.as_slice() else {
        panic!("it cites one provision, got {:?}", case.cites);
    };
    let edition = &cited.places[0].editions[0];
    assert_eq!(edition.written, "1976 ed.");
    assert!(
        edition.printings.is_empty(),
        "no printing from 1976 is held"
    );
    assert_eq!(cited.since.edition_not_held.as_deref(), Some("1976 ed."));
    assert!(
        cited.since.windows.is_empty(),
        "a window would compare the provision now at that number, got {:?}",
        cited.since.windows
    );
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_not_call_the_provision_unchanged_when_cases_citing_reads_an_edition_not_held() {
    let output = run(&[
        "cases-citing",
        dataset_path(),
        "--cites",
        "8 U.S.C. \u{a7} 1105a(a)",
    ]);
    assert!(
        output.status.success(),
        "cases-citing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains(
            "no window is given: the opinion cites the Code of 1976, written \"(1976 ed.)\" \
             after the citation, and this dataset holds no printing of title 8 from that year."
        ),
        "the edition is named, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("unchanged") && !stdout.contains("CHANGED"),
        "no window is given, got:\n{stdout}"
    );
}

/// The opinion of the Court in *Windsor* cites "8 U.S.C. § 1186a(b)(1) (2006
/// ed. and Supp. V)": an edition and a supplement in one pair of brackets.
#[test]
fn should_name_the_year_and_the_words_written_when_cases_citing_reads_an_edition_with_a_supplement()
{
    let output = run(&[
        "cases-citing",
        dataset_path(),
        "--cites",
        "8 U.S.C. \u{a7} 1186a(b)(1)",
    ]);
    let stdout = stdout_of(&output);

    assert!(
        stdout.contains(
            "no window is given: the opinion cites the Code of 2006, written \"(2006 ed. and \
             Supp. V)\" after the citation, and this dataset holds no printing of title 8 from \
             that year."
        ),
        "the year and the words written are named, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("of the Code"),
        "the words written are not put in front of \"of the Code\", got:\n{stdout}"
    );
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "cases-citing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A printing of title 26, and two opinions that write only a year after a
/// citation (#470): *United Stationers, Inc. v. United States* (N.D. Ill.
/// 1997), "26 U.S.C. § 41 (1986)", and the opinion of case 6248 (1994),
/// "26 U.S.C. § 162 (1988)". Both are added by `add-opinions` offline from
/// the committed CourtListener records.
fn title_26_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/edition_year_only.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and two opinions that write a year".to_string(),
            ..Default::default()
        });
        let file = "tests/test_data/usc/2025-07-18/usc26.xml";
        dataset
            .add_uslm_xml(file, "2025-07-18", None)
            .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        let added = run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--opinions",
            "1527901,6248",
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the opinions, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

#[test]
fn should_name_the_year_when_the_opinion_writes_only_a_year_after_the_citation() {
    let output = run(&[
        "cases-citing",
        title_26_path(),
        "--cites",
        "26 U.S.C. \u{a7} 41",
    ]);
    let stdout = stdout_of(&output);

    assert!(
        stdout.contains(
            "the opinion cites the Code of 1986, written \"(1986)\" after the citation. This \
             dataset holds no printing of title 26 from 1986, so that law is out of scope."
        ),
        "the year is named, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("the 1986 of the Code"),
        "no word is missing, got:\n{stdout}"
    );
}

#[test]
fn should_name_the_year_on_the_line_with_no_window_when_the_opinion_writes_only_a_year() {
    let output = run(&[
        "cases-citing",
        title_26_path(),
        "--cites",
        "26 U.S.C. \u{a7} 162",
    ]);
    let stdout = stdout_of(&output);

    assert!(
        stdout.contains(
            "no window is given: the opinion cites the Code of 1988, written \"(1988)\" after \
             the citation, and this dataset holds no printing of title 26 from that year."
        ),
        "the year is named, got:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "the opinion cites the Code of 1988, written \"(1988)\" after the citation. This \
             dataset holds no printing of title 26 from 1988, so that law is out of scope."
        ),
        "the line on the place names the year the same way, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("the 1988 of the Code"),
        "no word is missing, got:\n{stdout}"
    );
}

/// What `cases-citing` says of an edition the dataset does not hold, as `path`
/// and `settle --explain` must say it too (#414).
const WARNING_1976: &str = "the opinion cites the Code of 1976, written \"(1976 ed.)\" after the \
citation. This dataset holds no printing of title 8 from 1976, so that law is out of scope. A \
section number can be used again, so the provision now at that number can be a different \
provision from the one the court read.";

/// The link from the *Windsor* dissent to § 1105a(a), by its short id.
fn link_to_1105a_a() -> String {
    let links = citation_links();
    let link = links
        .iter()
        .find(|link| link.object.path() == Some(SECTION_1105A_A))
        .expect("one passage cites § 1105a(a)");
    words_to_data::review::short_id(&link.id()).to_string()
}

#[test]
fn should_warn_under_the_link_when_path_reads_a_citation_of_an_edition_not_held() {
    let output = run(&["path", dataset_path(), SECTION_1105A_A]);
    let stdout = stdout_of(&output);

    let link = format!("[link {}]", link_to_1105a_a());
    let lines: Vec<&str> = stdout.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.contains(&link))
        .unwrap_or_else(|| panic!("path lists the link, got:\n{stdout}"));
    assert_eq!(
        lines.get(at + 1).map(|line| line.trim()),
        Some(WARNING_1976),
        "the warning is the line under the link, got:\n{stdout}"
    );
}

#[test]
fn should_give_the_edition_on_the_link_when_path_json_reads_a_citation_of_an_edition() {
    let output = run(&["path", dataset_path(), SECTION_1105A_A, "--json"]);
    let report: serde_json::Value =
        serde_json::from_str(&stdout_of(&output)).expect("path --json should print JSON");

    let link = link_to_1105a_a();
    let [citation] = report["citations"]
        .as_array()
        .expect("the report lists citations")
        .iter()
        .filter(|citation| citation["id"] == link.as_str())
        .collect::<Vec<_>>()
        .try_into()
        .expect("the report lists the link once");
    assert_eq!(
        citation["editions"],
        serde_json::json!([{ "written": "1976 ed.", "year": "1976", "printings": [] }]),
        "the edition is a field of the link"
    );
}

#[test]
fn should_warn_before_the_evidence_when_settle_explains_a_citation_of_an_edition_not_held() {
    let output = run(&[
        "settle",
        dataset_path(),
        "--link",
        &link_to_1105a_a(),
        "--explain",
    ]);
    let stdout = stdout_of(&output);

    let warning = stdout
        .find(WARNING_1976)
        .unwrap_or_else(|| panic!("settle --explain gives the warning, got:\n{stdout}"));
    let evidence = stdout
        .find("How it was made:")
        .unwrap_or_else(|| panic!("settle --explain shows the evidence, got:\n{stdout}"));
    assert!(
        warning < evidence,
        "the warning comes before the evidence, got:\n{stdout}"
    );
}

/// A link of the title 26 fixture that names no edition, to a provision that
/// no link with an edition cites: its short id and the cited path.
fn link_with_no_edition() -> (String, String) {
    let links = Dataset::open_sqlite(title_26_path())
        .expect("the fixture should open")
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read");
    let names_edition = |link: &Link| {
        link.payload.as_ref().is_some_and(|payload| {
            payload.value["occurrences"]
                .as_array()
                .is_some_and(|places| places.iter().any(|place| place.get("edition").is_some()))
        })
    };
    let with_edition: Vec<&str> = links
        .iter()
        .filter(|link| names_edition(link))
        .filter_map(|link| link.object.path())
        .collect();
    let link = links
        .iter()
        .find(|link| {
            !names_edition(link)
                && link
                    .object
                    .path()
                    .is_some_and(|path| !with_edition.iter().any(|cited| cited.starts_with(path)))
        })
        .expect("the fixture holds a citation with no edition");
    (
        words_to_data::review::short_id(&link.id()).to_string(),
        link.object
            .path()
            .expect("a citation names a path")
            .to_string(),
    )
}

#[test]
fn should_print_no_edition_when_path_and_settle_read_a_citation_that_names_none() {
    let (link, cited) = link_with_no_edition();

    let shown = stdout_of(&run(&["path", title_26_path(), &cited]));
    assert!(
        shown.contains(&format!("[link {link}]")),
        "path lists the link, got:\n{shown}"
    );
    let explained = stdout_of(&run(&[
        "settle",
        title_26_path(),
        "--link",
        &link,
        "--explain",
    ]));
    for stdout in [&shown, &explained] {
        assert!(
            !stdout.contains("the opinion cites the Code of")
                && !stdout.contains("A section number can be used again"),
            "no edition is said, got:\n{stdout}"
        );
    }

    let report: serde_json::Value = serde_json::from_str(&stdout_of(&run(&[
        "path",
        title_26_path(),
        &cited,
        "--json",
    ])))
    .expect("path --json should print JSON");
    for citation in report["citations"].as_array().expect("citations") {
        assert!(
            citation.get("editions").is_none(),
            "a link that names no edition has no editions field, got {citation}"
        );
    }
}
