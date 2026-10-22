//! `cases-citing` below the section: the subdivision an opinion wrote, and the
//! subdivision a question asks about, read at read time (#302).
//!
//! A `judicial.cites` link targets the section. The subdivision the opinion
//! wrote, such as the `(a)(1)` of `26 U.S.C. § 174(a)(1)`, is kept only in the
//! link's payload. These tests pin that the query reads it, resolves it against
//! the printing it reads, and never says "does not cite" about an opinion that
//! cites a path above the one asked about.
//!
//! The fixture is real: two printings of title 26 from the committed corpus, and
//! the ten committed CourtListener records, added by the `add-opinions` command
//! offline. Of the nine that cite § 174, five wrote a subdivision that the held
//! printings hold — `(a)(1)` four times, `(a)` once — three wrote the section
//! alone, and *Shami* wrote `(e)`, which no held printing has: § 174 runs from
//! (a) to (d) in 2025. *Snow* writes `§ 174 (a) (1)`, with a space before each
//! bracket, and is read as `(a)(1)` since #309.

use std::collections::BTreeSet;
use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::citation::resolve::Subdivision;
use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::judicial::reliance::{self, Citing, CitingCase, Relation};
use words_to_data::storage::SqliteStorage;

const TITLE_26_EARLIER: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_LATER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";

/// The ten opinions, as the command takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Two printings of title 26 and the ten opinions, built once for the file.
///
/// The opinions go in through `add-opinions`, so the links are the links the
/// command writes, payload and all.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/cases_citing_subdivision.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and ten opinions".to_string(),
            description: "Two printings of title 26, for citations below the section".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        for (file, date) in [
            (TITLE_26_EARLIER, "2025-07-18"),
            (TITLE_26_LATER, "2025-07-30"),
        ] {
            dataset
                .add_uslm_xml(file, date, None)
                .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");

        let added = run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            CACHE,
            "--opinions",
            TEN,
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the ten, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

fn dataset() -> Dataset<SqliteStorage> {
    Dataset::open_sqlite(dataset_path()).expect("the fixture should open")
}

/// The writing that cites, by its path.
fn work_of(case: &CitingCase) -> String {
    match &case.citing {
        Citing::Held { path, .. } => path.clone(),
        Citing::NotHeld { reference, .. } => reference.clone(),
    }
}

/// The works whose citation stands in this relation to the path asked about.
fn works_where(cases: &[CitingCase], relation: Relation) -> BTreeSet<String> {
    cases
        .iter()
        .filter(|case| case.cites.iter().any(|cited| cited.relation == relation))
        .map(work_of)
        .collect()
}

fn set(works: &[&str]) -> BTreeSet<String> {
    works.iter().map(|work| work.to_string()).collect()
}

#[test]
fn should_list_an_opinion_as_citing_a_subdivision_when_the_subdivision_it_wrote_resolves_there() {
    let asked = format!("{SECTION_174}/subsection_a/paragraph_1");

    let cases = reliance::cases_citing(&dataset(), &asked).expect("the query should run");

    assert_eq!(
        works_where(&cases, Relation::Within),
        set(&[
            "judicial/case_109019/opinion_109019",
            "judicial/case_122262/opinion_9434364",
            "judicial/case_122262/opinion_9434365",
            "judicial/case_6248/opinion_6248",
        ]),
        "Snow, Boeing, the Boeing dissent and Harris wrote § 174(a)(1)"
    );
}

#[test]
fn should_list_an_opinion_as_citing_a_path_that_holds_the_asked_path_when_it_wrote_no_subdivision_below_it()
 {
    let asked = format!("{SECTION_174}/subsection_a/paragraph_1");

    let cases = reliance::cases_citing(&dataset(), &asked).expect("the query should run");

    assert_eq!(
        works_where(&cases, Relation::Holds),
        set(&[
            // The section alone.
            "judicial/case_1527901/opinion_1527901",
            "judicial/case_7029371/opinion_6931314",
            "judicial/case_8998794/opinion_8991218",
            // `(e)`, which no held printing has, so only the section is known.
            "judicial/case_2651100/opinion_2651100",
            // `(a)`, which holds `(a)(1)`.
            "judicial/case_406879/opinion_406879",
        ]),
        "an opinion that cites a path above the one asked about is never \"does not cite\""
    );
    assert!(
        !cases
            .iter()
            .any(|case| work_of(case) == "judicial/case_2812209/opinion_2812209"),
        "Obergefell cites no part of § 174"
    );
}

#[test]
fn should_leave_out_an_opinion_whose_subdivision_resolves_to_another_branch_when_a_subdivision_is_asked_about()
 {
    let asked = format!("{SECTION_174}/subsection_b");

    let cases = reliance::cases_citing(&dataset(), &asked).expect("the query should run");

    let reached: BTreeSet<String> = cases.iter().map(work_of).collect();
    for wrote_a in [
        "judicial/case_109019/opinion_109019",
        "judicial/case_122262/opinion_9434364",
        "judicial/case_122262/opinion_9434365",
        "judicial/case_6248/opinion_6248",
        "judicial/case_406879/opinion_406879",
    ] {
        assert!(
            !reached.contains(wrote_a),
            "{wrote_a} wrote § 174(a), which is not (b) and does not hold it, got {reached:?}"
        );
    }
    assert!(works_where(&cases, Relation::Within).is_empty());
    assert_eq!(works_where(&cases, Relation::Holds).len(), 4);
}

/// The one citation an opinion makes to § 174, asked about at the section.
fn citation_by(work: &str) -> reliance::CitedProvision {
    let cases = reliance::cases_citing(&dataset(), SECTION_174).expect("the query should run");
    let case = cases
        .into_iter()
        .find(|case| work_of(case) == work)
        .unwrap_or_else(|| panic!("{work} cites § 174"));
    assert_eq!(case.cites.len(), 1, "{work} has one link to § 174");
    case.cites.into_iter().next().expect("one citation")
}

#[test]
fn should_measure_a_change_window_at_the_written_subdivision_when_it_resolves_in_the_printings() {
    let paragraph_1 = format!("{SECTION_174}/subsection_a/paragraph_1");

    // Boeing wrote § 174(a)(1). Between the two printings § 174 changed at
    // (a), (a)(2)(B), (b) and (d), and not at (a)(1).
    let boeing = citation_by("judicial/case_122262/opinion_9434364");
    assert_eq!(
        boeing.subdivision,
        Subdivision::Resolved {
            resolved_path: paragraph_1.clone()
        }
    );
    let window = &boeing.since.windows[0];
    assert_eq!(
        window.at, paragraph_1,
        "the window is read where the opinion pointed"
    );
    assert!(
        !window.changed,
        "(a)(1) did not change in this window, got {:?}",
        window.changed_paths
    );

    // Encyclopaedia Britannica wrote § 174(a), and (a) did change.
    let britannica = citation_by("judicial/case_406879/opinion_406879");
    let subsection_a = format!("{SECTION_174}/subsection_a");
    assert_eq!(britannica.since.windows[0].at, subsection_a);
    assert!(britannica.since.windows[0].changed);
}

#[test]
fn should_measure_a_change_window_at_the_whole_section_when_the_written_subdivision_is_not_held() {
    // Shami wrote § 174(e). No held printing has an (e): § 174 runs from (a) to
    // (d) in 2025.
    let shami = citation_by("judicial/case_2651100/opinion_2651100");

    assert_eq!(
        shami.subdivision,
        Subdivision::Unresolved {
            unresolved: "(e)".to_string()
        },
        "the part that does not resolve is named"
    );
    let window = &shami.since.windows[0];
    assert_eq!(
        window.at, SECTION_174,
        "the window covers the whole section"
    );
    assert!(window.changed, "and the section did change in it");
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "cases-citing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// The cases listed under the "no writing that cites it" line, and nothing
/// else.
fn listed_as_not_citing(printed: &str) -> BTreeSet<String> {
    printed
        .lines()
        .skip_while(|line| !line.contains("have no writing that cites it"))
        .skip(1)
        .take_while(|line| line.starts_with("  judicial/"))
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn should_print_the_opinions_that_cite_a_path_above_the_asked_path_in_their_own_group() {
    let asked = format!("{SECTION_174}/subsection_a/paragraph_1");

    let printed = stdout_of(&run(&["cases-citing", dataset_path(), "--path", &asked]));

    assert!(
        printed.contains("4 opinion(s) cite it:"),
        "Snow, Boeing, its dissent and Harris wrote (a)(1), got:\n{printed}"
    );
    assert!(
        printed.contains("5 opinion(s) cite a provision that holds this path"),
        "the section alone, (e) and (a) hold (a)(1), got:\n{printed}"
    );
    assert_eq!(
        listed_as_not_citing(&printed),
        set(&["judicial/case_2812209"]),
        "only Obergefell cites no part of § 174, got:\n{printed}"
    );
}

/// The answer, from the first `===` line on. What comes before it is how the
/// question was resolved, which differs between `--cites` and `--path`.
fn answer_of(printed: &str) -> String {
    let start = printed.find("=== ").expect("the answer starts with ===");
    printed[start..].to_string()
}

#[test]
fn should_answer_a_citation_with_a_subdivision_as_the_path_it_resolves_to() {
    let asked = format!("{SECTION_174}/subsection_a/paragraph_1");

    let by_path = stdout_of(&run(&["cases-citing", dataset_path(), "--path", &asked]));
    let by_citation = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--cites",
        "26 U.S.C. \u{a7} 174(a)(1)",
    ]));

    assert_eq!(
        answer_of(&by_citation),
        answer_of(&by_path),
        "the (a)(1) of the citation is not dropped"
    );
}

#[test]
fn should_name_the_part_of_a_citation_it_cannot_resolve_when_the_printing_does_not_hold_it() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--cites",
        "26 U.S.C. \u{a7} 174(e)",
    ]));

    assert!(
        printed.contains("(e) of 174(e) does not resolve"),
        "the (e) is not dropped without a note, got:\n{printed}"
    );
    assert!(
        answer_of(&printed).starts_with(&format!("=== {SECTION_174}\n")),
        "and the answer is about the whole section, got:\n{printed}"
    );
}

/// The first repro of #302. § 174 has no (e) in any held printing, so nothing
/// can resolve to it; the opinions that cite § 174 are still not "does not
/// cite", and the reader is told the path is not held.
#[test]
fn should_say_the_asked_path_is_in_no_printing_and_keep_the_section_citations_when_it_is_not_held()
{
    let asked = format!("{SECTION_174}/subsection_e");

    let printed = stdout_of(&run(&["cases-citing", dataset_path(), "--path", &asked]));

    assert!(
        printed.contains("No printing this dataset holds has this path"),
        "got:\n{printed}"
    );
    assert!(
        !printed.contains("No opinion in this dataset cites it."),
        "four opinions cite the section that holds it, got:\n{printed}"
    );
    assert!(
        printed.contains("4 opinion(s) cite a provision that holds this path"),
        "the three that wrote § 174 alone, and Shami's (e), got:\n{printed}"
    );
    assert_eq!(
        listed_as_not_citing(&printed),
        set(&[
            "judicial/case_2812209",
            // § 174(a) and § 174(a)(1): another branch of the section.
            "judicial/case_109019",
            "judicial/case_122262",
            "judicial/case_6248",
            "judicial/case_406879",
        ]),
        "got:\n{printed}"
    );
}

/// The one citation `work` makes, from the `--json` answer to one question.
fn json_citation<'a>(answer: &'a serde_json::Value, work: &str) -> &'a serde_json::Value {
    let cases = answer["cases"].as_array().expect("cases is a list");
    let case = cases
        .iter()
        .find(|case| case["citing"]["held"]["path"] == work)
        .unwrap_or_else(|| panic!("{work} is in the answer: {answer:#}"));
    &case["cites"][0]
}

#[test]
fn should_carry_the_resolved_path_and_whether_it_fell_back_to_the_section_in_json() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_174,
        "--json",
    ]));
    let answers: serde_json::Value =
        serde_json::from_str(&printed).unwrap_or_else(|error| panic!("{error}: {printed}"));
    let answer = &answers[0];
    assert_eq!(answer["path"], SECTION_174);

    let harris = json_citation(answer, "judicial/case_6248/opinion_6248");
    assert_eq!(harris["resolution"], "resolved");
    assert_eq!(
        harris["resolved_path"],
        format!("{SECTION_174}/subsection_a/paragraph_1")
    );
    assert_eq!(harris["relation"], "within");

    let shami = json_citation(answer, "judicial/case_2651100/opinion_2651100");
    assert_eq!(shami["resolution"], "unresolved");
    assert_eq!(shami["unresolved"], "(e)");
    assert_eq!(shami["since"]["windows"][0]["at"], SECTION_174);

    let united_stationers = json_citation(answer, "judicial/case_1527901/opinion_1527901");
    assert_eq!(united_stationers["resolution"], "section");

    assert_eq!(
        answer["not_citing"],
        serde_json::json!(["judicial/case_2812209"])
    );
}
