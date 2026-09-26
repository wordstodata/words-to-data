//! Reviews: a verdict on a link, recorded as a link of its own.
//!
//! Every link reviewed here is a real one, written out of the committed corpus
//! by the redesignation reading. No model runs and nothing is invented: the
//! bill states the renumbering, the reading places it, and the review argues
//! with the link that came out.
//!
//! **The window is `2025-07-18 -> 2025-07-30`.** The corpus holds three release
//! points, so it holds two windows, and `119-hr-1` reached the Code in this
//! one.
//!
//! `docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`
//! holds the shape and why each part of it is where it is.

use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::link::{Link, LinkKind, Named};
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The committed public law, which holds every redesignation in the corpus.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code: the window every
/// case here is read over.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// § 898(c), whose paragraph (3) the bill renumbered as paragraph (2).
const SUBSECTION_898_C: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_II/subpart_D/section_898/subsection_c";

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

/// Title 26 at both release points, with the redesignations `119-hr-1` states
/// recorded as links over the one window.
fn dataset_with_real_links() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (file, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        dataset
            .add_uslm_xml(file, date, None)
            .expect("title 26 should load");
    }
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the bill should load");

    let work = WorkId::new("uscode/title_26");
    let from = ExpressionId::new(work.clone(), BEFORE);
    let to = ExpressionId::new(work, AFTER);
    let stated =
        words_to_data::uslm::bill_redesignation::redesignations_stated_in_file(BILL_ID, BILL)
            .expect("the bill should parse");
    dataset
        .record_redesignations(BILL_ID, &stated, &from, &to)
        .expect("the redesignations should record");
    dataset
}

/// The real link this file argues with: paragraph (3) of § 898(c) became
/// paragraph (2).
fn reviewed_link(dataset: &Dataset<InMemoryStorage>) -> Link {
    dataset
        .links_for_path(&format!("{SUBSECTION_898_C}/paragraph_3"))
        .expect("links should read")
        .into_iter()
        .find(|link| link.kind.0 == LinkKind::REDESIGNATED_AS)
        .expect("paragraph (3) carries a redesignation link")
}

/// A moment, as a reviewer's clock would give it.
fn at(date: &str) -> time::OffsetDateTime {
    time::OffsetDateTime::new_utc(
        time::Date::parse(date, &time::format_description::well_known::Iso8601::DATE)
            .expect("the date should parse"),
        time::Time::MIDNIGHT,
    )
}

/// Two reviewers of one link leave two records.
///
/// This is the case the whole shape exists for. The reviewer sits in the
/// hashed object rather than in the provenance, because a provenance is not
/// hashed: two reviewers named only there would mint one id, and `add_link`,
/// finding the first review's human-touched provenance, returns `Ok(())` and
/// drops the second. The rule that protects a review would eat a review.
#[test]
fn should_leave_two_records_when_two_reviewers_review_one_link() {
    let mut dataset = dataset_with_real_links();
    let reviewed = reviewed_link(&dataset);

    let jesse = Review {
        verdict: Verdict::Refuted,
        reviewer: "human:jesse".to_string(),
        reasoning: Some("The bill renumbers (3) as (2) and this names (4).".to_string()),
        at: at("2026-09-26"),
    };
    let model = Review {
        verdict: Verdict::Confirmed,
        reviewer: "model:local".to_string(),
        reasoning: Some("§ 70352(a) states it in those words.".to_string()),
        at: at("2026-09-25"),
    };
    dataset
        .add_link(jesse.about(&reviewed))
        .expect("the first review should record");
    dataset
        .add_link(model.about(&reviewed))
        .expect("the second review should record");

    let records = dataset
        .reviews_of(&reviewed.id())
        .expect("the reviews should read");

    assert_eq!(
        records.len(),
        2,
        "each reviewer left a record: {records:#?}"
    );
    let mut reviewers: Vec<String> = records
        .iter()
        .map(|record| record.provenance.source.clone())
        .collect();
    reviewers.sort();
    assert_eq!(reviewers, vec!["human:jesse", "model:local"]);
    let mut kinds: Vec<String> = records.iter().map(|record| record.kind.0.clone()).collect();
    kinds.sort();
    assert_eq!(
        kinds,
        vec![LinkKind::REVIEW_CONFIRMED, LinkKind::REVIEW_REFUTED],
        "the verdict is in the kind, so it is hashed"
    );
}

/// A review record with no timestamp is refused.
///
/// The newest review of a link is the one a reader reports, so a record with no
/// timestamp could never be ordered and no reader would ever see it. Writing it
/// would look like settling a link and settle nothing.
#[test]
fn should_refuse_the_record_when_a_review_carries_no_timestamp() {
    let mut dataset = dataset_with_real_links();
    let reviewed = reviewed_link(&dataset);

    let mut undated = Review {
        verdict: Verdict::Refuted,
        reviewer: "human:jesse".to_string(),
        reasoning: Some("The renumbering is stated the other way round.".to_string()),
        at: at("2026-09-26"),
    }
    .about(&reviewed);
    undated.provenance.timestamp = None;

    let refused = review::record(&mut dataset, undated)
        .expect_err("a review with no timestamp should be refused");

    assert!(
        matches!(refused, review::Refused::NoTimestamp),
        "it says which rule it broke, got {refused:?}"
    );
    assert!(
        refused.to_string().contains("timestamp"),
        "and says so in words: {refused}"
    );
    assert!(
        dataset
            .reviews_of(&reviewed.id())
            .expect("the reviews should read")
            .is_empty(),
        "a refused review is not written"
    );
}

/// One reviewer who changes their mind leaves both records, and a reader
/// reports the newer.
///
/// The verdict sits in the hashed kind, so the second record cannot overwrite
/// the first. Precedence is the reading and never the record: there is no
/// withdrawal, no supersession and no configured trust order, and publishing
/// over a verdict is the whole of the fix (ADR 0012).
#[test]
fn should_keep_both_records_and_report_the_newer_when_a_reviewer_changes_their_mind() {
    let mut dataset = dataset_with_real_links();
    let reviewed = reviewed_link(&dataset);
    let reviewer = "human:jesse";

    for (verdict, day, why) in [
        (
            Verdict::Refuted,
            "2026-09-25",
            "I read it the other way round.",
        ),
        (
            Verdict::Confirmed,
            "2026-09-26",
            "§ 70352(a) states exactly this.",
        ),
    ] {
        let review = Review {
            verdict,
            reviewer: reviewer.to_string(),
            reasoning: Some(why.to_string()),
            at: at(day),
        };
        review::record(&mut dataset, review.about(&reviewed)).expect("the review should record");
    }

    let records = dataset
        .reviews_of(&reviewed.id())
        .expect("the reviews should read");
    assert_eq!(
        records.len(),
        2,
        "the earlier verdict survives the later one: {records:#?}"
    );

    let winning = review::newest(&records).expect("one review should win");
    assert_eq!(winning.verdict, Verdict::Confirmed);
    assert_eq!(winning.reviewer, reviewer);
    assert_eq!(winning.at, at("2026-09-26"));
    assert_eq!(
        winning.reasoning.as_deref(),
        Some("§ 70352(a) states exactly this."),
        "the winning record carries the reason it was made for"
    );
}

/// Every link the dataset holds, over every kind.
fn every_link(dataset: &Dataset<InMemoryStorage>) -> Vec<Link> {
    dataset
        .count_links_by_kind()
        .expect("the kinds should count")
        .into_keys()
        .flat_map(|kind| {
            dataset
                .links_by_kind(&kind)
                .expect("the links of a kind should read")
        })
        .collect()
}

/// A link is named by a prefix of its id, git-style.
///
/// A link id is a sha256 of what the link says, so a prefix of it is
/// reproducible across a rebuild and across datasets and a reviewer can type it
/// (ADR 0004). Any length is accepted: nothing forces a reviewer to paste
/// sixty-four characters.
#[test]
fn should_name_one_link_when_a_prefix_of_its_id_is_given() {
    let dataset = dataset_with_real_links();
    let wanted = reviewed_link(&dataset).id();

    let printed = review::short_id(&wanted);
    let named = dataset
        .link_by_id_prefix(printed)
        .expect("the prefix should resolve");

    match named {
        Named::One(found) => assert_eq!(found.id(), wanted),
        other => panic!("the printed id should name one link, got {other:?}"),
    }
}

/// An ambiguous prefix is refused, and the answer says how many it matched.
///
/// The count is the whole value of the refusal: it tells the reviewer they must
/// type more of the id, which a bare "no" would not. The prefix here is one
/// character, and which character it is comes out of the corpus rather than out
/// of a guess.
#[test]
fn should_refuse_the_prefix_and_count_the_matches_when_more_than_one_link_matches() {
    let dataset = dataset_with_real_links();
    let ids: Vec<String> = every_link(&dataset).iter().map(Link::id).collect();
    assert!(
        ids.len() > 1,
        "the corpus should write more than one link, got {}",
        ids.len()
    );

    // A first character two of the real ids share. Sixteen are possible and the
    // corpus writes tens of links, so one is always shared.
    let shared = ids
        .iter()
        .map(|id| &id[..1])
        .find(|start| ids.iter().filter(|id| id.starts_with(*start)).count() > 1)
        .expect("two ids should share a first character");
    let sharing = ids.iter().filter(|id| id.starts_with(shared)).count();

    let named = dataset
        .link_by_id_prefix(shared)
        .expect("the prefix should resolve");

    assert_eq!(
        named,
        Named::Ambiguous(sharing),
        "the refusal names how many links `{shared}` matched"
    );
}

/// A dataset of real links written to disk, and the short id of the link this
/// file argues with.
///
/// Written fresh for each caller under its own name, so one run's review cannot
/// reach another run's fixture.
fn fixture_on_disk(name: &str) -> (String, String, String) {
    let dataset = dataset_with_real_links();
    let reviewed = reviewed_link(&dataset);
    let input = format!("{}/{name}.json", env!("CARGO_TARGET_TMPDIR"));
    let written = format!("{}/{name}_reviewed.json", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&input, Format::Compact)
        .expect("the fixture should save");
    (input, written, reviewed.id())
}

/// One run of `settle`, and what it said.
fn settle(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg("settle")
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Every review record a written dataset holds about one link.
fn reviews_in(path: &str, link_id: &str) -> Vec<Link> {
    Dataset::<InMemoryStorage>::load(path, Format::Compact)
        .expect("the written dataset should load")
        .reviews_of(link_id)
        .expect("the reviews should read")
}

/// The command settles a link named by a short id prefix, over a real dataset.
///
/// The link is one the redesignation reading wrote out of the committed corpus
/// over `2025-07-18 -> 2025-07-30`. No model runs.
#[test]
fn should_record_a_review_when_the_command_settles_a_link_by_its_short_id() {
    let (input, written, reviewed_id) = fixture_on_disk("settle_one");
    let printed = review::short_id(&reviewed_id).to_string();

    let output = settle(&[
        &input,
        "--link",
        &printed,
        "--verdict",
        "refuted",
        "--reviewer",
        "human:jesse",
        "--reason",
        "The bill renumbers (3) as (2) and this link has it the other way round.",
        "--output",
        &written,
    ]);

    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        said.contains(&printed),
        "it names the link it settled by its printed id: {said}"
    );
    assert!(
        said.contains("refuted") && said.contains("human:jesse"),
        "and names the verdict and the reviewer: {said}"
    );

    let records = reviews_in(&written, &reviewed_id);
    assert_eq!(records.len(), 1, "one review was written: {records:#?}");
    assert_eq!(records[0].kind.0, LinkKind::REVIEW_REFUTED);
    assert_eq!(records[0].provenance.source, "human:jesse");
    assert!(
        records[0].provenance.timestamp.is_some(),
        "the run's clock sets the timestamp, so newest-wins can order it"
    );
    assert_eq!(
        records[0]
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.as_deref()),
        Some("The bill renumbers (3) as (2) and this link has it the other way round."),
        "the reviewer's stated reason travels with the record"
    );
}

/// The command refuses an ambiguous prefix, and says how many links matched.
///
/// A bare refusal would leave the reviewer guessing how much more of the id to
/// type. Nothing is written, so a run that could have settled the wrong link
/// settles none.
#[test]
fn should_refuse_and_say_how_many_matched_when_the_command_is_given_an_ambiguous_prefix() {
    let (input, written, reviewed_id) = fixture_on_disk("settle_ambiguous");
    let shared = &reviewed_id[..1];

    let output = settle(&[
        &input,
        "--link",
        shared,
        "--verdict",
        "refuted",
        "--reviewer",
        "human:jesse",
        "--reason",
        "One character cannot name a link.",
        "--output",
        &written,
    ]);

    assert!(!output.status.success(), "the run should stop");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains(&format!("`{shared}` names ")),
        "the refusal names the prefix: {said}"
    );
    let matched: usize = said
        .split_whitespace()
        .find_map(|word| word.parse().ok())
        .unwrap_or_else(|| panic!("the refusal should name a count: {said}"));
    assert!(
        matched > 1,
        "and the count is the number of links it matched, got {matched}: {said}"
    );
    assert!(
        said.contains("more of the id"),
        "and says what to do about it: {said}"
    );
    assert!(
        !std::path::Path::new(&written).exists(),
        "a run that settles nothing writes nothing"
    );
}

/// A dataset of real links with one of them reviewed on a named date, written
/// to disk.
///
/// The review is recorded through the library rather than the command, because
/// the command reads the run's clock and a report's wording is what is under
/// test here.
fn fixture_reviewed(name: &str, reviews: &[(Verdict, &str, &str)]) -> String {
    let mut dataset = dataset_with_real_links();
    let reviewed = reviewed_link(&dataset);
    for (verdict, reviewer, day) in reviews {
        let review = Review {
            verdict: *verdict,
            reviewer: (*reviewer).to_string(),
            reasoning: Some("Read against § 70352(a) of the bill.".to_string()),
            at: at(day),
        };
        review::record(&mut dataset, review.about(&reviewed)).expect("the review should record");
    }
    let path = format!("{}/{name}.json", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");
    path
}

/// `path` names the winning review's verdict, its source and its date.
///
/// Before this the report printed only the stored verification state, so a
/// dataset holding a review read exactly like one holding none. That is the
/// reader-that-lies defect of #220 and #226, which is why the door and the
/// reading ship together.
#[test]
fn should_name_the_winning_reviews_verdict_source_and_date_when_path_reports_a_reviewed_link() {
    let dataset = fixture_reviewed(
        "path_reviewed",
        &[(Verdict::Confirmed, "human:jesse", "2026-09-26")],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "path",
            &dataset,
            &format!("{SUBSECTION_898_C}/paragraph_3"),
            "--from",
            &format!("uscode/title_26@{BEFORE}"),
            "--to",
            &format!("uscode/title_26@{AFTER}"),
        ])
        .output()
        .expect("the binary should run");

    assert!(
        output.status.success(),
        "the command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains("machine suggested"),
        "the stored trust is still printed: {said}"
    );
    assert!(
        said.contains("confirmed by human:jesse on 2026-09-26"),
        "and the winning review's verdict, source and date beside it: {said}"
    );
}

/// One run of `path` over § 898(c)'s paragraph (3), across the one window.
fn path_over(dataset: &str, at_path: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "path",
            dataset,
            at_path,
            "--from",
            &format!("uscode/title_26@{BEFORE}"),
            "--to",
            &format!("uscode/title_26@{AFTER}"),
        ])
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "the command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// A step whose newest review is a refutation is not followed.
///
/// Following it would state something a reviewer checked and found wrong, which
/// is worse than the path-string pairing the walk replaces. The refusal is said
/// aloud rather than left silent: a reader who got a clean answer would have no
/// way to learn a link was passed over (#165).
#[test]
fn should_refuse_the_step_when_the_newest_review_of_the_link_is_a_refutation() {
    let dataset = fixture_reviewed(
        "path_refuted",
        &[(Verdict::Refuted, "human:jesse", "2026-09-26")],
    );

    let said = path_over(&dataset, &format!("{SUBSECTION_898_C}/paragraph_3"));

    assert!(
        !said.contains("moved out to"),
        "the walk does not follow a refuted step: {said}"
    );
    assert!(
        said.contains("Redesignations not followed"),
        "and it says that it did not: {said}"
    );
    assert!(
        said.contains("refuted by human:jesse on 2026-09-26"),
        "and names who refuted it and when: {said}"
    );
}

/// A refutation a later reviewer confirmed over does not stop the walk.
///
/// This is newest-wins reaching the reader. Precedence is the **reading** and
/// never the record: the refutation is still in the dataset, and it is no longer
/// the one a reader reports. Any reviewer may override any other — an agent may
/// override a human — because the program ranks nobody (ADR 0012).
#[test]
fn should_follow_the_step_when_a_later_review_confirms_over_an_earlier_refutation() {
    let dataset = fixture_reviewed(
        "path_refuted_then_confirmed",
        &[
            (Verdict::Refuted, "human:jesse", "2026-09-25"),
            (Verdict::Confirmed, "model:local", "2026-09-26"),
        ],
    );

    let said = path_over(&dataset, &format!("{SUBSECTION_898_C}/paragraph_3"));

    assert!(
        said.contains("moved out to"),
        "the newer verdict is the one read, so the walk goes on: {said}"
    );
    assert!(
        said.contains("confirmed by model:local on 2026-09-26"),
        "and the report names the winning record: {said}"
    );
    assert!(
        !said.contains("Redesignations not followed"),
        "nothing was passed over: {said}"
    );
}

/// Two reviewers who agree about one link are not a contradiction.
///
/// A review copies the reviewed link's subject as a locator, so two reviewers
/// of one link are two links with one subject — which is the shape
/// `contradictions` groups on. Grouped that way they come out as a
/// *disagreement*, because the reviewer sits in each object and the two objects
/// therefore differ. Two reviewers agreeing would be reported as two statements
/// that cannot both be true, and that is the reader-that-lies defect of #220
/// and #226 arriving by a new road.
///
/// So `contradictions` reports contradictions **about the law** and leaves the
/// `review` namespace alone. Reading a pair of reviews is a different rule, and
/// #179 holds it open.
#[test]
fn should_report_no_contradiction_when_two_reviewers_agree_about_one_link() {
    let dataset = fixture_reviewed(
        "contradictions_two_reviewers",
        &[
            (Verdict::Confirmed, "human:jesse", "2026-09-25"),
            (Verdict::Confirmed, "model:local", "2026-09-26"),
        ],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["contradictions", &dataset, "--json"])
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "the command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the command should emit json");

    for category in ["duplication", "disagreement"] {
        let groups = report[category].as_array().expect("a list");
        assert!(
            groups.iter().all(|group| group["kind"]
                .as_str()
                .is_some_and(|kind| !kind.starts_with(LinkKind::REVIEW))),
            "no group should be made of reviews, got {category}:\n{groups:#?}"
        );
    }
}

/// A review is stored by a dataset at schema 10, in place, with no format
/// change.
///
/// This is the whole claim of ADR 0012 put to a real file. A review is
/// expressed entirely in the kind, which ADR 0002 made an open namespaced
/// string precisely so a party could add a link type without permission, so no
/// `Target` variant, no `VerificationState` variant and no promoted column was
/// needed. A database also grows in place, which is the form in which a review
/// survives `add-release-points`.
#[test]
fn should_hold_the_review_at_schema_ten_when_a_database_is_settled_in_place() {
    assert_eq!(
        words_to_data::storage::SCHEMA_VERSION,
        10,
        "a review costs no format change, so the schema does not move"
    );

    let path = format!("{}/settle_in_place.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let dataset = dataset_with_real_links();
    let reviewed_id = reviewed_link(&dataset).id();
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");

    let output = settle(&[
        &path,
        "--link",
        review::short_id(&reviewed_id),
        "--verdict",
        "disputed",
        "--reviewer",
        "model:local",
        "--reason",
        "The clause may be about the subsection above.",
    ]);
    assert!(
        output.status.success(),
        "the command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Re-opened, so the record is read out of the file and not out of memory.
    // A dataset whose schema had moved would be refused by name here.
    let reopened = Dataset::open_sqlite(&path).expect("the database should reopen");
    let records = reopened
        .reviews_of(&reviewed_id)
        .expect("the reviews should read");
    assert_eq!(records.len(), 1, "the review is in the file: {records:#?}");
    assert_eq!(records[0].kind.0, LinkKind::REVIEW_DISPUTED);

    let winning = review::newest(&records).expect("one review should win");
    assert_eq!(winning.verdict, Verdict::Disputed);
    assert_eq!(winning.reviewer, "model:local");

    // And it comes back beside the link it reviews, which is what the copied
    // subject is for: no new query and no new promoted column.
    let beside = reopened
        .links_for_path(&format!("{SUBSECTION_898_C}/paragraph_3"))
        .expect("links should read");
    assert!(
        beside
            .iter()
            .any(|link| link.kind.0 == LinkKind::REVIEW_DISPUTED),
        "the review sits beside the link it reviews: {beside:#?}"
    );
}
