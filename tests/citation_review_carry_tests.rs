//! The case law rebuild carries each review of a citation link to the new
//! links that say the same thing (#319,
//! `docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decision 9).
//!
//! A rebuild gives every citation link a new id, because the id is a hash of
//! the subject and the object (ADR 0004), and ADR 0014 moves both. A review
//! names the link it reviews by that id (ADR 0012). So without a carry, every
//! review of a citation stays in the old dataset and names nothing in the new
//! one.
//!
//! **The old links are real.** `tests/test_data/review_carry/citation_links_before_313.json`
//! holds the 39 `judicial.cites` links of a dataset built on 2026-10-21, before
//! #313 made a case the work. Each subject is `judicial/opinion_<id>`, so every
//! one of them has a new id today. The new dataset is titles 1 and 26 and the ten
//! committed opinions, added with `add-opinions --offline`, as
//! `cli_opinions_tests.rs` builds it.

mod common;

use std::process::{Command, Output};

use words_to_data::citation::carry::Carried;
use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::link::{Link, LinkKind};
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The ten opinions, as `add-opinions` takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

/// The citation links of a dataset built before #313.
const OLD_LINKS: &str = "tests/test_data/review_carry/citation_links_before_313.json";

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn metadata(name: &str) -> DatasetMetadata {
    DatasetMetadata {
        name: name.to_string(),
        description: "Citation links and their reviews".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

fn path_for(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    path
}

/// The rebuilt dataset: titles 1 and 26 as they read on 2025-07-18, and the ten
/// committed opinions added to it.
fn rebuilt(name: &str) -> String {
    let path = path_for(name);
    let mut dataset: Dataset<InMemoryStorage> = Dataset::new(metadata("Rebuilt"));
    for title in ["usc01.xml", "usc26.xml"] {
        common::add_uslm_xml(
            &mut dataset,
            &format!("tests/test_data/usc/2025-07-18/{title}"),
            "2025-07-18",
        );
    }
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    stdout_of(&run(&[
        "add-opinions",
        &path,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        TEN,
    ]));
    path
}

/// The citation links of the dataset built before #313.
fn old_links() -> Vec<Link> {
    let text = std::fs::read_to_string(OLD_LINKS).expect("the old links should be committed");
    serde_json::from_str(&text).expect("the old links should read as links")
}

/// The old link from `subject` to the section at `object`.
fn old_link(subject: &str, object: &str) -> Link {
    old_links()
        .into_iter()
        .find(|link| link.subject.path() == Some(subject) && link.object.path() == Some(object))
        .unwrap_or_else(|| panic!("the old dataset should hold {subject} -> {object}"))
}

/// The old dataset: its citation links, and a review of some of them.
fn old_with_reviews(name: &str, reviews: &[(Link, Review)]) -> String {
    let path = path_for(name);
    let mut dataset: Dataset<InMemoryStorage> = Dataset::new(metadata("Old"));
    for link in old_links() {
        dataset.add_link(link).expect("an old link should store");
    }
    for (reviewed, said) in reviews {
        review::record(&mut dataset, said.about(reviewed)).expect("the review should record");
    }
    dataset
        .save_to_sqlite(&path)
        .expect("the old dataset should save");
    path
}

/// The time the tester reviewed, 2026-10-22 at 10:00 UTC.
fn morning_of_2026_10_22() -> time::OffsetDateTime {
    time::OffsetDateTime::new_utc(
        time::Date::from_calendar_date(2026, time::Month::October, 22).expect("a real date"),
        time::Time::from_hms(10, 0, 0).expect("a real time"),
    )
}

fn confirmed_by_tester() -> Review {
    Review {
        verdict: Verdict::Confirmed,
        reviewer: "human:tester".to_string(),
        reasoning: Some("The opinion cites § 174(a)(1) here.".to_string()),
        at: morning_of_2026_10_22(),
    }
}

/// The citation links in the rebuilt dataset from `subject` to `object`.
fn new_links(dataset: &str, subject: &str, object: &str) -> Vec<Link> {
    let dataset = Dataset::open_sqlite(dataset).expect("the rebuilt dataset should open");
    dataset
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
        .into_iter()
        .filter(|link| link.subject.path() == Some(subject) && link.object.path() == Some(object))
        .collect()
}

/// Every review record in `dataset` that names `link`.
fn reviews_of(dataset: &str, link: &Link) -> Vec<Link> {
    let dataset = Dataset::open_sqlite(dataset).expect("the rebuilt dataset should open");
    dataset
        .reviews_of(&link.id())
        .expect("the reviews should read")
}

#[test]
fn should_carry_a_confirmation_to_the_new_link_when_the_rebuild_moved_the_citing_writing() {
    // Snow v. Commissioner cites § 174(a)(1). Before #313 the writing was
    // `judicial/opinion_109019`; now it is a child of its case.
    let reviewed = old_link("judicial/opinion_109019", SECTION_174);
    let old = old_with_reviews(
        "carry_snow_old",
        &[(reviewed.clone(), confirmed_by_tester())],
    );
    let new = rebuilt("carry_snow_new");
    let [renamed] = new_links(&new, "judicial/case_109019/opinion_109019", SECTION_174)
        .try_into()
        .expect("the rebuild holds one link from Snow's writing to § 174");
    assert_ne!(
        renamed.id(),
        reviewed.id(),
        "the rebuild gave the link a new id"
    );

    stdout_of(&run(&["carry-reviews", &new, "--from", &old]));

    let records = reviews_of(&new, &renamed);
    let winning = review::newest(&records).expect("the new link carries the review");
    assert_eq!(winning.verdict, Verdict::Confirmed);
    assert_eq!(
        winning.reviewer, "human:tester",
        "the reviewer is still the reviewer"
    );
    assert_eq!(
        winning.at,
        morning_of_2026_10_22(),
        "and the time is theirs"
    );
    let carried = Carried::read(&records[0]).expect("the record says it was carried");
    assert_eq!(carried.link, reviewed.id(), "the record names the old link");
}

const SECTION_994: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_A/section_994";

#[test]
fn should_carry_a_refutation_to_the_writing_that_wrote_the_same_form_when_the_old_record_is_now_only_a_source()
 {
    // Before #313, Boeing was the combined record 122262, and it cited
    // § 994 as "994(a)(2)". Now 122262 is only a source, and the writings are
    // the split records: the Court's opinion (9434364) writes "994(a)(2)", and
    // the dissent (9434365) writes "994". Only the first says the same thing.
    let reviewed = old_link("judicial/opinion_122262", SECTION_994);
    let refuted = Review {
        verdict: Verdict::Refuted,
        reviewer: "human:tester".to_string(),
        reasoning: Some("A test refutation.".to_string()),
        at: morning_of_2026_10_22(),
    };
    let old = old_with_reviews("carry_boeing_old", &[(reviewed.clone(), refuted)]);
    let new = rebuilt("carry_boeing_new");
    let [majority] = new_links(&new, "judicial/case_122262/opinion_9434364", SECTION_994)
        .try_into()
        .expect("the Court's opinion cites § 994 once");
    let [dissent] = new_links(&new, "judicial/case_122262/opinion_9434365", SECTION_994)
        .try_into()
        .expect("the dissent cites § 994 once");

    stdout_of(&run(&["carry-reviews", &new, "--from", &old]));

    let records = reviews_of(&new, &majority);
    let winning = review::newest(&records).expect("the Court's opinion carries the review");
    assert_eq!(winning.verdict, Verdict::Refuted);
    assert_eq!(
        Carried::read(&records[0]).map(|carried| carried.link),
        Some(reviewed.id())
    );
    assert!(
        reviews_of(&new, &dissent).is_empty(),
        "the dissent wrote another form, so the review is not about its link"
    );
}

const SECTION_1361: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_S/part_I/section_1361";

#[test]
fn should_report_a_review_and_write_nothing_when_no_new_link_says_what_the_old_link_said() {
    // Before #313 the opinion 2651100 cited § 1361 as "1361(a)(1)". The
    // rebuild reads another text of it, and holds no citation of § 1361 by it.
    let reviewed = old_link("judicial/opinion_2651100", SECTION_1361);
    let old = old_with_reviews(
        "carry_unmatched_old",
        &[(reviewed.clone(), confirmed_by_tester())],
    );
    let new = rebuilt("carry_unmatched_new");
    let before = Dataset::open_sqlite(&new)
        .expect("the rebuilt dataset should open")
        .links_by_namespace(LinkKind::REVIEW)
        .expect("the reviews should read")
        .len();

    let printed = stdout_of(&run(&["carry-reviews", &new, "--from", &old, "--json"]));

    let report: serde_json::Value = serde_json::from_str(&printed).expect("--json prints JSON");
    assert_eq!(report["carried"], 0);
    assert_eq!(
        report["not_carried"], 1,
        "the review is reported: {report:#}"
    );
    let row = &report["reviews"][0];
    assert_eq!(row["old_link"], reviewed.id());
    assert_eq!(row["statement"]["record"], "2651100");
    assert_eq!(row["statement"]["written"][0], "1361(a)(1)");
    assert_eq!(row["new_links"].as_array().map(Vec::len), Some(0));
    assert!(
        row["why"]
            .as_str()
            .is_some_and(|why| why.contains("no citation of uscode/title_26/section_1361")),
        "the row says why: {row:#}"
    );
    let after = Dataset::open_sqlite(&new)
        .expect("the rebuilt dataset should open")
        .links_by_namespace(LinkKind::REVIEW)
        .expect("the reviews should read")
        .len();
    assert_eq!(
        after, before,
        "nothing is written for a review with no new link"
    );

    let printed = stdout_of(&run(&["carry-reviews", &new, "--from", &old]));
    assert!(
        printed.contains("1 review(s) have no new link"),
        "the text report counts it: {printed}"
    );
}

#[test]
fn should_copy_the_review_as_it_is_when_the_rebuild_made_the_same_link_again() {
    // Two builds of today's dataset make the same link from Snow's writing to
    // § 174, with the same id. The review needs no carry: the same record
    // names the right link in both (#295, its first case).
    let old = rebuilt("carry_same_old");
    let [reviewed] = new_links(&old, "judicial/case_109019/opinion_109019", SECTION_174)
        .try_into()
        .expect("today's build holds one link from Snow's writing to § 174");
    let written = confirmed_by_tester().about(&reviewed);
    {
        let mut dataset = Dataset::open_sqlite(&old).expect("the old dataset should open");
        review::record(&mut dataset, written.clone()).expect("the review should record");
    }
    let new = rebuilt("carry_same_new");

    let printed = stdout_of(&run(&["carry-reviews", &new, "--from", &old, "--json"]));

    let records = reviews_of(&new, &reviewed);
    assert_eq!(
        records,
        [written],
        "the record is copied as it is: {printed}"
    );
}

#[test]
fn should_carry_a_review_to_the_writing_a_rule_found_when_the_old_link_named_the_whole_record() {
    // Before #313, Obergefell was one combined record, 2812209, and it cited
    // 1 U.S.C. § 7. Now a rule splits that record into its writings
    // (#315), and the Court's opinion, `opinion_2812209-1`, holds the citation.
    let reviewed = old_link(
        "judicial/opinion_2812209",
        "uscode/title_1/chapter_1/section_7",
    );
    let old = old_with_reviews(
        "carry_split_old",
        &[(reviewed.clone(), confirmed_by_tester())],
    );
    let new = rebuilt("carry_split_new");
    let [court] = new_links(
        &new,
        "judicial/case_2812209/opinion_2812209-1",
        "uscode/title_1/chapter_1/section_7",
    )
    .try_into()
    .expect("the Court's opinion cites 1 U.S.C. § 7 once");

    stdout_of(&run(&["carry-reviews", &new, "--from", &old]));

    let records = reviews_of(&new, &court);
    let winning = review::newest(&records).expect("the Court's opinion carries the review");
    assert_eq!(winning.verdict, Verdict::Confirmed);
}
