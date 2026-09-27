//! `info` says how much of a dataset has been reviewed, by link kind.
//!
//! A dataset can hold verdicts, and before this nothing counted them: an agent
//! could not tell whether 1011 amendment links held one judgement or a thousand
//! (#236).
//!
//! **The figure is a census and not a gate.** It lives in `info`, which has no
//! exit code to abuse. `validate` fails a run when it finds outstanding work,
//! and an unreviewed link is work a person finishes by making a judgement, not
//! work a command finishes by running — so `validate` would exit 1 for the life
//! of the project and become a command nobody runs.
//!
//! **And the `review` namespace is left out.** A review is a link of its own,
//! it mirrors the reviewed link's subject so ordinary queries return it beside
//! what it reviews, and it carries no reviews itself — so its honest status is
//! `Unreviewed` (`docs/adr/0012`). A total that counted review records would
//! therefore **grow** as reviewing proceeds, and a progress report that goes up
//! as you make progress is worse than no report.
//!
//! The links here are real: the committed output of one matching run over the
//! real corpus, so no model runs and nothing is invented.

use time::{Date, Month, OffsetDateTime, Time};
use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::inspect;
use words_to_data::link::{Link, LinkKind};
use words_to_data::query::LinkQuery;
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The committed output of one real matching run.
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";
const TITLE_26: &str = "uscode/title_26";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// Every amendment link the run recorded over title 26, and no documents.
///
/// `info` counts links, so the text is not needed and parsing two release
/// points of title 26 would cost 112 MB of XML for nothing.
fn amendment_links() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");

    let work = WorkId::new(TITLE_26);
    let from = ExpressionId::new(work.clone(), BEFORE);
    let to = ExpressionId::new(work, AFTER);

    let mut dataset = Dataset::new(DatasetMetadata::default());
    for annotation in &annotations {
        if !annotation
            .paths
            .iter()
            .all(|path| path.starts_with(TITLE_26))
        {
            continue;
        }
        for link in Link::from_annotation(annotation, &from, &to) {
            dataset.add_link(link).expect("the link should be added");
        }
    }
    dataset
}

/// A fresh dataset answers "none reviewed" rather than saying nothing.
///
/// The section is present and every link sits in the unreviewed bucket, because
/// "no judgement has been made yet" is the answer an agent needs before it
/// starts. Omitting the section would read as "this tool does not measure that".
#[test]
fn should_report_every_kind_as_fully_unreviewed_when_a_dataset_holds_no_reviews() {
    let dataset = amendment_links();

    let info = inspect::info(&dataset).expect("info should read");

    // Guard, so the loop below cannot pass over an empty map.
    assert!(
        !info.link_counts_by_kind.is_empty(),
        "the fixture should hold links of at least one kind"
    );
    assert_eq!(
        info.review_states_by_kind.keys().collect::<Vec<_>>(),
        info.link_counts_by_kind.keys().collect::<Vec<_>>(),
        "every kind held should carry a review-state row"
    );
    for (kind, held) in &info.link_counts_by_kind {
        let states = &info.review_states_by_kind[kind];
        assert_eq!(
            states.unreviewed, *held,
            "{kind}: every link should be unreviewed"
        );
        assert_eq!(states.confirmed, 0, "{kind}: nothing was confirmed");
        assert_eq!(states.refuted, 0, "{kind}: nothing was refuted");
        assert_eq!(states.disputed, 0, "{kind}: nothing was disputed");
    }
}

/// Midnight UTC on one day, which is when the reviews below were made.
fn at(year: i32, month: Month, day: u8) -> OffsetDateTime {
    OffsetDateTime::new_utc(
        Date::from_calendar_date(year, month, day).expect("a real date"),
        Time::MIDNIGHT,
    )
}

/// One real amendment link of the fixture, so a review can argue with it.
///
/// Taken by position out of the kind's own rows rather than by path, because the
/// cases below need two different links and do not care which two.
fn nth_amendment_link(dataset: &Dataset<InMemoryStorage>, nth: usize) -> Link {
    let amendments = LinkQuery::new().of_kind(LinkKind::AMENDED_BY);
    dataset
        .links_matching(&amendments)
        .expect("the query should read")
        .rows
        .get(nth)
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "the fixture should hold at least {} amendment links",
                nth + 1
            )
        })
}

/// Record one verdict about one link, through the one door a review goes
/// through.
fn record(
    dataset: &mut Dataset<InMemoryStorage>,
    link: &Link,
    verdict: Verdict,
    reviewer: &str,
    when: OffsetDateTime,
) {
    let review = Review {
        verdict,
        reviewer: reviewer.to_string(),
        reasoning: Some("Read against the bill.".to_string()),
        at: when,
    };
    review::record(dataset, review.about(link)).expect("the review should record");
}

/// The report never counts the review records themselves.
///
/// A review is a link of a kind, so the moment one is recorded the dataset holds
/// a `review.confirmed` kind and `link_counts_by_kind` names it. A review
/// carries no reviews of its own, so a status query honestly calls it
/// `Unreviewed` — and counting it would make the outstanding-work figure
/// **climb** as the reviewing gets done.
#[test]
fn should_leave_the_review_namespace_out_of_the_breakdown_when_a_dataset_holds_reviews() {
    let mut dataset = amendment_links();
    let reviewed = nth_amendment_link(&dataset, 0);
    record(
        &mut dataset,
        &reviewed,
        Verdict::Confirmed,
        "human:jesse",
        at(2026, Month::September, 26),
    );

    let info = inspect::info(&dataset).expect("info should read");

    // Guard: the dataset really does hold a review kind, so the assertion below
    // cannot pass because nothing was recorded.
    assert!(
        info.link_counts_by_kind
            .keys()
            .any(|kind| LinkKind::new(kind.clone()).namespace() == LinkKind::REVIEW),
        "the link counts should name the review kind just recorded, got {:?}",
        info.link_counts_by_kind.keys().collect::<Vec<_>>()
    );
    let counted_reviews: Vec<&String> = info
        .review_states_by_kind
        .keys()
        .filter(|kind| LinkKind::new((*kind).clone()).namespace() == LinkKind::REVIEW)
        .collect();
    assert!(
        counted_reviews.is_empty(),
        "no review kind should carry a review-state row, got {counted_reviews:?}"
    );
}

/// The heading the human output prints the breakdown under.
const HEADING: &str = "Review state:";

/// The fixture with one amendment link confirmed and another refuted.
///
/// Two different verdicts, so a line of four counts cannot pass by printing the
/// same number four times.
fn one_confirmed_and_one_refuted() -> Dataset<InMemoryStorage> {
    let mut dataset = amendment_links();
    let confirmed = nth_amendment_link(&dataset, 0);
    let refuted = nth_amendment_link(&dataset, 1);
    assert_ne!(
        confirmed.id(),
        refuted.id(),
        "the fixture should hold two different amendment links"
    );
    record(
        &mut dataset,
        &confirmed,
        Verdict::Confirmed,
        "human:jesse",
        at(2026, Month::September, 26),
    );
    record(
        &mut dataset,
        &refuted,
        Verdict::Refuted,
        "human:jesse",
        at(2026, Month::September, 26),
    );
    dataset
}

/// Save a dataset and run `info` against it the way a person would.
///
/// The dataset holds links and no documents, so a compact file is the whole of
/// it: `info` counts links, and parsing two release points of title 26 would
/// cost 112 MB of XML for nothing.
fn info_run(dataset: &Dataset<InMemoryStorage>, args: &[&str]) -> std::process::Output {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.json");
    let path = file.to_str().expect("a utf-8 path");
    dataset
        .save(path, Format::Compact)
        .expect("the fixture should save");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg("info")
        .arg(path)
        .args(args)
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "info should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// What `info` printed for a person to read.
fn info_stdout(dataset: &Dataset<InMemoryStorage>) -> String {
    String::from_utf8_lossy(&info_run(dataset, &[]).stdout).to_string()
}

/// The indented lines under the breakdown's heading, and nothing else.
fn review_state_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip_while(|line| *line != HEADING)
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .map(str::to_string)
        .collect()
}

/// A person reading `info` sees how much of each kind is judged.
///
/// One line for each kind, naming all four states. `validate` is deliberately
/// not the place for this: an unreviewed link is work a person finishes by
/// making a judgement, so `validate` would exit 1 for the life of the project.
#[test]
fn should_print_each_kind_and_its_four_counts_when_info_prints_for_a_person() {
    let dataset = one_confirmed_and_one_refuted();
    let held = inspect::info(&dataset)
        .expect("info should read")
        .link_counts_by_kind[LinkKind::AMENDED_BY];

    let stdout = info_stdout(&dataset);

    let lines = review_state_lines(&stdout);
    assert_eq!(
        lines.len(),
        1,
        "one line for the one kind under review, got {lines:?} in:\n{stdout}"
    );
    let line = &lines[0];
    for expected in [
        LinkKind::AMENDED_BY.to_string(),
        format!("{} unreviewed", held - 2),
        "1 confirmed".to_string(),
        "1 refuted".to_string(),
        "0 disputed".to_string(),
    ] {
        assert!(
            line.contains(&expected),
            "the line should name {expected:?}, got {line:?}"
        );
    }
}

/// A machine reader gets the same figures a person gets.
///
/// Two outputs of one report, so they must not be able to disagree. A `--json`
/// that carried a different number would make the two readers argue about one
/// dataset.
#[test]
fn should_carry_the_same_figures_in_json_as_in_the_printed_output_when_info_runs() {
    let dataset = one_confirmed_and_one_refuted();

    let printed = review_state_lines(&info_stdout(&dataset));
    let json: serde_json::Value = serde_json::from_slice(&info_run(&dataset, &["--json"]).stdout)
        .expect("info --json should emit json");

    let states = json["review_states_by_kind"]
        .as_object()
        .expect("--json should carry the breakdown");
    assert_eq!(
        states.len(),
        printed.len(),
        "json should name the same kinds the printed output names, got {states:?}"
    );
    for (kind, counts) in states {
        let read = |name: &str| counts[name].as_u64().expect("a count");
        let expected = format!(
            "  {kind}  {} unreviewed, {} confirmed, {} refuted, {} disputed",
            read("unreviewed"),
            read("confirmed"),
            read("refuted"),
            read("disputed")
        );
        assert!(
            printed
                .iter()
                .any(|line| line.trim_end() == expected.trim_end()),
            "the printed output should carry {expected:?}, got {printed:?}"
        );
    }
}

/// The states of one kind, as `info` reports them.
fn states_of(dataset: &Dataset<InMemoryStorage>, kind: &str) -> inspect::ReviewStates {
    inspect::info(dataset)
        .expect("info should read")
        .review_states_by_kind
        .get(kind)
        .cloned()
        .unwrap_or_else(|| panic!("{kind} should carry a review-state row"))
}

/// Every link stays counted exactly once, so the four states add up.
fn total_of(states: &inspect::ReviewStates) -> usize {
    states.unreviewed + states.confirmed + states.refuted + states.disputed
}

/// One judgement moves one link, and moves it out of the outstanding work.
#[test]
fn should_move_one_link_from_unreviewed_to_confirmed_when_a_confirming_verdict_is_recorded() {
    let mut dataset = amendment_links();
    let before = states_of(&dataset, LinkKind::AMENDED_BY);

    let reviewed = nth_amendment_link(&dataset, 0);
    record(
        &mut dataset,
        &reviewed,
        Verdict::Confirmed,
        "human:jesse",
        at(2026, Month::September, 26),
    );

    let after = states_of(&dataset, LinkKind::AMENDED_BY);
    assert_eq!(
        after.confirmed, 1,
        "the one judged link should be confirmed"
    );
    assert_eq!(
        after.unreviewed,
        before.unreviewed - 1,
        "the outstanding work should fall by exactly one"
    );
}

/// Two reviewers who disagree still leave one link, in one bucket.
///
/// A link is counted where its **newest** review puts it, so the older verdict
/// does not also get a count. Counting both would report more links of a kind
/// than the dataset holds.
#[test]
fn should_count_a_link_once_in_its_newest_verdict_when_two_reviews_disagree() {
    let mut dataset = amendment_links();
    let held = inspect::info(&dataset)
        .expect("info should read")
        .link_counts_by_kind[LinkKind::AMENDED_BY];
    let reviewed = nth_amendment_link(&dataset, 0);

    record(
        &mut dataset,
        &reviewed,
        Verdict::Confirmed,
        "model:local",
        at(2026, Month::September, 25),
    );
    record(
        &mut dataset,
        &reviewed,
        Verdict::Refuted,
        "human:jesse",
        at(2026, Month::September, 26),
    );

    let states = states_of(&dataset, LinkKind::AMENDED_BY);
    assert_eq!(states.refuted, 1, "the newer verdict refutes it");
    assert_eq!(
        states.confirmed, 0,
        "the older verdict should not carry a count of its own"
    );
    assert_eq!(
        total_of(&states),
        held,
        "the four states should add up to the links held, and never above it"
    );
}

/// The report never grows as the reviewing gets done.
///
/// This is the whole reason the `review` namespace is left out. A review is a
/// link, so each one raises `link_count`; if the breakdown counted them, the
/// figure an agent reads as "work left" would climb with every judgement made.
#[test]
fn should_keep_the_total_unchanged_when_a_link_is_reviewed() {
    let mut dataset = amendment_links();
    let before = states_of(&dataset, LinkKind::AMENDED_BY);
    let links_before = inspect::info(&dataset)
        .expect("info should read")
        .link_count;

    let reviewed = nth_amendment_link(&dataset, 0);
    record(
        &mut dataset,
        &reviewed,
        Verdict::Disputed,
        "human:jesse",
        at(2026, Month::September, 26),
    );

    let after_info = inspect::info(&dataset).expect("info should read");
    // Guard: the review really did land, so the assertion below is about the
    // breakdown holding still rather than about nothing having happened.
    assert_eq!(
        after_info.link_count,
        links_before + 1,
        "the review should have been added as a link of its own"
    );

    let after = states_of(&dataset, LinkKind::AMENDED_BY);
    assert_eq!(
        total_of(&after),
        total_of(&before),
        "reviewing a link should raise no total, got {after:?} against {before:?}"
    );
    assert_eq!(
        after.disputed, 1,
        "the judged link should have moved into the disputed bucket"
    );
    assert_eq!(
        after_info.review_states_by_kind.len(),
        1,
        "the review's own kind should add no row, got {:?}",
        after_info.review_states_by_kind.keys().collect::<Vec<_>>()
    );
}
