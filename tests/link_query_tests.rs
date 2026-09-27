//! One query vocabulary over links, asked as a **product** of filters.
//!
//! Before this, `AnnotationQuery` was a sum — `Pair | Bill | Path` — so a
//! reviewer could name a window **or** a bill **or** a path, never a
//! conjunction. *"What did this bill change at § 174 in this window"* was
//! unaskable, and the way through was to diff a whole title and grep the output
//! (#234).
//!
//! The links here are real: the committed output of one matching run over the
//! real corpus, so no model runs and nothing is invented.

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::link::{Link, LinkKind};
use words_to_data::query::{LinkQuery, Locator, PathMatch, ReviewStatus};
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{InMemoryStorage, LinkReader};

use time::{Date, Month, OffsetDateTime, Time};

/// The committed output of one real matching run.
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";
const TITLE_26: &str = "uscode/title_26";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// Subsection (a) of § 174, which more than one amendment of the bill changed.
///
/// One amendment narrows it to foreign research; another reaches it through
/// § 864(g)(2). Two links at one path, from two amendments, is what makes a
/// conjunction provable: filtering by the path alone cannot tell them apart.
/// The bill the committed matching run was made over.
const BILL_ID: &str = "119-21";

/// Section 174, whose subtree the amendments above sit in.
const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

const SECTION_174_A: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";

/// Every amendment link the run recorded over title 26, and no documents.
///
/// A query filters links, so the text is not needed and parsing two release
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

/// A query naming a path **and** an object returns only what matches both.
///
/// The case the sum type could not express. Two amendments changed § 174(a), so
/// the path alone cannot name one of them, and the object alone reaches every
/// path that amendment touched. Only the conjunction names one link.
#[test]
fn should_return_only_the_links_matching_both_when_a_query_names_a_path_and_an_object() {
    let dataset = amendment_links();

    let at_path = LinkQuery::new().at(Locator::new().at_path(SECTION_174_A, PathMatch::Exact));
    let everything_at_path = dataset
        .links_matching(&at_path)
        .expect("the query should read");

    // Guard, so the conjunction below cannot pass on a single-link path: the
    // fixture must really hold more than one amendment at this provision.
    assert!(
        everything_at_path.total > 1,
        "the fixture should hold several amendment links at § 174(a), found {}",
        everything_at_path.total
    );

    let one_amendment = everything_at_path.rows[0].object.name();
    let both = LinkQuery::new()
        .at(Locator::new().at_path(SECTION_174_A, PathMatch::Exact))
        .with_object_prefix(&one_amendment);
    let narrowed = dataset
        .links_matching(&both)
        .expect("the query should read");

    assert!(
        narrowed.total >= 1,
        "the conjunction should still find the link it was built from"
    );
    assert!(
        narrowed.total < everything_at_path.total,
        "naming the object as well should narrow: {} of {} at the path",
        narrowed.total,
        everything_at_path.total
    );
    assert!(
        narrowed
            .rows
            .iter()
            .all(|link| link.object.name() == one_amendment),
        "every row should match the object the query named"
    );
}

/// A query naming nothing returns everything, and says how many there are.
///
/// Not a refusal, which is what `annotations` does today and which makes
/// exploring a dataset hostile. The total is carried beside the rows so a
/// truncated run can say what it dropped rather than reading as complete (#235).
#[test]
fn should_return_every_link_and_its_total_when_a_query_names_no_filter() {
    let dataset = amendment_links();

    let answer = dataset
        .links_matching(&LinkQuery::new())
        .expect("the query should read");

    let held: usize = dataset
        .count_links_by_kind()
        .expect("the kinds should count")
        .values()
        .sum();
    assert_eq!(
        answer.total, held,
        "an unfiltered query should account for every link the dataset holds"
    );
    assert_eq!(
        answer.rows.len(),
        answer.total,
        "and with no limit it should return them all"
    );
}

/// A limit truncates the rows and leaves the total alone.
///
/// The total is what makes "… and N more" sayable, so it must count the matches
/// rather than the rows returned. A limit that also capped the total would make
/// a truncated answer read as a complete one.
#[test]
fn should_keep_the_total_when_a_limit_truncates_the_rows() {
    let dataset = amendment_links();

    let answer = dataset
        .links_matching(&LinkQuery::new().with_limit(3))
        .expect("the query should read");

    assert_eq!(answer.rows.len(), 3, "the rows should be limited");
    assert!(
        answer.total > 3,
        "the total should count every match, got {}",
        answer.total
    );
    assert_eq!(
        answer.dropped(),
        answer.total - 3,
        "and the answer should say how many it did not return"
    );
}

/// A query can ask for the links nobody has reviewed.
///
/// This is the term that made the reviewer's real question askable: *"unreviewed
/// amendment links for this bill"* is status **and** kind **and** object, and
/// leaving status out of the vocabulary would have left it unaskable after all
/// the rest of the work (#234, item 3).
///
/// Status is the one filter with no index behind it. A link's review state is
/// derived from the reviews pointing at it, so it is computed rather than looked
/// up (`docs/adr/0007`).
#[test]
fn should_tell_reviewed_links_from_unreviewed_ones_when_a_query_names_a_status() {
    let mut dataset = amendment_links();
    // Named by kind, and this is not incidental. A review **is** a link, and it
    // copies the reviewed link's subject so that existing queries return it
    // beside what it reviews (`docs/adr/0012`). So a status query naming no kind
    // also matches the review records themselves, which carry no reviews of
    // their own and are honestly `Unreviewed`. The reviewer's real question is
    // "unreviewed *amendment* links", and naming the kind is what makes it one
    // question rather than two.
    let amendments = LinkQuery::new().of_kind(LinkKind::AMENDED_BY);
    let all = dataset
        .links_matching(&amendments)
        .expect("the query should read")
        .total;

    let at_path = dataset
        .links_matching(
            &LinkQuery::new().at(Locator::new().at_path(SECTION_174_A, PathMatch::Exact)),
        )
        .expect("the query should read");
    assert!(
        at_path.total > 1,
        "the fixture should hold several links at § 174(a)"
    );
    let confirmed_link = at_path.rows[0].clone();
    let refuted_link = at_path.rows[1].clone();

    for (link, verdict) in [
        (&confirmed_link, Verdict::Confirmed),
        (&refuted_link, Verdict::Refuted),
    ] {
        let review = Review {
            verdict,
            reviewer: "human:jesse".to_string(),
            reasoning: Some("Read against the bill.".to_string()),
            at: OffsetDateTime::new_utc(
                Date::from_calendar_date(2026, Month::September, 26).expect("a real date"),
                Time::MIDNIGHT,
            ),
        };
        review::record(&mut dataset, review.about(link)).expect("the review should record");
    }

    let counted = |status| {
        dataset
            .links_matching(
                &LinkQuery::new()
                    .of_kind(LinkKind::AMENDED_BY)
                    .with_status(status),
            )
            .expect("the query should read")
            .total
    };

    assert_eq!(
        counted(ReviewStatus::Confirmed),
        1,
        "one link was confirmed"
    );
    assert_eq!(counted(ReviewStatus::Refuted), 1, "one link was refuted");
    assert_eq!(
        counted(ReviewStatus::Disputed),
        0,
        "nobody disputed anything"
    );
    assert_eq!(
        counted(ReviewStatus::Unreviewed),
        all - 2,
        "every other amendment link is unreviewed"
    );
}

/// Status composes with the rest, which is the point of the product.
#[test]
fn should_narrow_by_status_and_path_together_when_a_query_names_both() {
    let mut dataset = amendment_links();
    let at_path = dataset
        .links_matching(
            &LinkQuery::new()
                .at(Locator::new().at_path(SECTION_174_A, PathMatch::Exact))
                .of_kind(LinkKind::AMENDED_BY),
        )
        .expect("the query should read");
    let reviewed = at_path.rows[0].clone();

    let review = Review {
        verdict: Verdict::Refuted,
        reviewer: "model:opus-5".to_string(),
        reasoning: None,
        at: OffsetDateTime::new_utc(
            Date::from_calendar_date(2026, Month::September, 26).expect("a real date"),
            Time::MIDNIGHT,
        ),
    };
    review::record(&mut dataset, review.about(&reviewed)).expect("the review should record");

    // Three terms at once — path, kind and status — which is the conjunction the
    // sum type could not hold.
    let unreviewed_here = dataset
        .links_matching(
            &LinkQuery::new()
                .at(Locator::new().at_path(SECTION_174_A, PathMatch::Exact))
                .of_kind(LinkKind::AMENDED_BY)
                .with_status(ReviewStatus::Unreviewed),
        )
        .expect("the query should read");

    assert_eq!(
        unreviewed_here.total,
        at_path.total - 1,
        "the reviewed link should drop out of the unreviewed set at this path"
    );
    assert!(
        unreviewed_here
            .rows
            .iter()
            .all(|link| link.id() != reviewed.id()),
        "and the reviewed link should not be among them"
    );
}

/// `annotations` accepts a bill **and** a path, and returns the intersection.
///
/// The case #234 was filed for. Before this the two flags carried
/// `conflicts_with_all`, because `AnnotationQuery` was a sum and could hold only
/// one of them, so the command refused the combination with a usage error.
#[test]
fn should_return_the_intersection_when_annotations_is_given_a_bill_and_a_path() {
    let dataset = amendment_links();
    let path = format!("{}/annotations_product.json", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let by_bill = annotation_rows(&["annotations", &path, "--bill", BILL_ID, "--json"]);
    let by_both = annotation_rows(&[
        "annotations",
        &path,
        "--bill",
        BILL_ID,
        "--path",
        SECTION_174,
        "--json",
    ]);

    assert!(
        by_bill.len() > 1,
        "the bill should name many annotations, found {}",
        by_bill.len()
    );
    assert!(
        !by_both.is_empty(),
        "the bill and the path together should still name some"
    );
    assert!(
        by_both.len() < by_bill.len(),
        "naming the path as well should narrow: {} of {}",
        by_both.len(),
        by_bill.len()
    );
    for row in &by_both {
        let paths = row["paths"].as_array().expect("paths is an array");
        assert!(
            paths
                .iter()
                .any(|p| p.as_str().unwrap_or_default().starts_with(SECTION_174)),
            "every row should touch the path named, got {paths:?}"
        );
        assert_eq!(
            row["bill_id"].as_str(),
            Some(BILL_ID),
            "and come from the bill named"
        );
    }
}

/// A run that shows only some of its rows says how many it did not show.
///
/// A cap that stayed silent would read as "that is all there is", which is the
/// defect #220 was about, one reader along.
#[test]
fn should_say_how_many_it_did_not_show_when_a_limit_truncates_the_listing() {
    let dataset = amendment_links();
    let path = format!("{}/annotations_limit.json", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let output = run(&["annotations", &path, "--bill", BILL_ID, "--limit", "2"]);
    assert!(output.status.success(), "the command should exit zero");
    let said = String::from_utf8_lossy(&output.stdout);

    // Read the total the run reports rather than counting rows: counting rows is
    // exactly the mistake a limit makes possible.
    let total = annotation_total(&["annotations", &path, "--bill", BILL_ID, "--json"]);
    assert!(
        said.contains(&format!("{} more", total - 2)),
        "the run should name the {} rows it did not show, got:\n{said}",
        total - 2
    );
}

/// The command as a subprocess, the way an agent or a shell would run it.
fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// The total one `--json` run reported, which counts matches and not rows.
fn annotation_total(args: &[&str]) -> usize {
    let output = run(args);
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    json["total"].as_u64().expect("a total") as usize
}

/// The annotation rows one `--json` run emitted.
fn annotation_rows(args: &[&str]) -> Vec<serde_json::Value> {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    json["annotations"]
        .as_array()
        .expect("an annotations array")
        .clone()
}
