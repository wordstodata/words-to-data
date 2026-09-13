//! Links as the stored form, not a projection (#70).
//!
//! `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` records
//! why a link is shaped and named this way. The fixture is
//! `tests/test_data/processed/annotations.json`, the output of a real matching
//! run over the real corpus.

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::link::{KindPayload, Link, LinkKind, Target, VerificationState};

const ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";
/// The work the fixture's first annotation belongs to.
const TITLE_26: &str = "uscode/title_26";

fn real_annotations() -> Vec<ChangeAnnotation> {
    let json = std::fs::read_to_string(ANNOTATIONS).expect("the fixture should be readable");
    serde_json::from_str(&json).expect("the fixture should parse as annotations")
}

fn pair() -> (ExpressionId, ExpressionId) {
    (
        ExpressionId::new(WorkId::new(TITLE_26), "2025-07-18"),
        ExpressionId::new(WorkId::new(TITLE_26), "2025-07-30"),
    )
}

/// The first fixture annotation, which touches title 26.
fn title_26_annotation() -> ChangeAnnotation {
    real_annotations()
        .into_iter()
        .find(|a| a.paths.iter().any(|p| p.starts_with(TITLE_26)))
        .expect("the fixture should hold a title 26 annotation")
}

#[test]
fn should_make_a_change_the_subject_of_an_amendment_link() {
    let annotation = title_26_annotation();
    let (from, to) = pair();

    let links = Link::from_annotation(&annotation, &from, &to);
    let link = links.first().expect("at least one link");

    // A bare provision cannot say *when* it was amended, so the pair was
    // dropped and `get_annotations(from, to)` had nothing to answer from.
    match &link.subject {
        Target::Change {
            work,
            path,
            from_date,
            to_date,
        } => {
            assert_eq!(work.as_str(), TITLE_26);
            assert!(annotation.paths.contains(path));
            assert_eq!(from_date, "2025-07-18");
            assert_eq!(to_date, "2025-07-30");
        }
        other => panic!("the subject should name a change, got {other:?}"),
    }
}

#[test]
fn should_identify_a_link_by_what_it_says_rather_than_by_who_said_it() {
    let annotation = title_26_annotation();
    let (from, to) = pair();
    let link = Link::from_annotation(&annotation, &from, &to)
        .into_iter()
        .next()
        .expect("at least one link");

    // Restating a fact a machine already stated must update one link, not grow
    // the table. That is what makes a rebuild idempotent.
    let mut restated = link.clone();
    restated.provenance.source = "human:jesse".to_string();
    restated.provenance.verification = VerificationState::HumanConfirmed;
    assert_eq!(
        link.id(),
        restated.id(),
        "provenance is who said it, not what was said"
    );

    let mut other_object = link.clone();
    other_object.object = Target::External {
        reference: "legislature.amendment:119-21:something-else".to_string(),
        display: "a different amendment".to_string(),
    };
    assert_ne!(
        link.id(),
        other_object.id(),
        "a different object is a different statement"
    );
}

#[test]
fn should_carry_the_bill_in_the_amendment_reference() {
    let annotation = title_26_annotation();
    let (from, to) = pair();

    let links = Link::from_annotation(&annotation, &from, &to);
    let link = links.first().expect("at least one link");

    // A reader that does not know the legislature extension must still be able
    // to resolve what the reference points at. The amendment id alone needed a
    // separate `bill_id` column to be useful, which is cruft from before the
    // core and the extensions were separated.
    match &link.object {
        Target::External { reference, .. } => {
            assert!(
                reference.contains(&annotation.source_bill.bill_id),
                "the reference should name the bill, got {reference}"
            );
            assert!(
                reference.contains(&annotation.source_bill.amendment_id),
                "the reference should name the amendment, got {reference}"
            );
        }
        other => panic!("an amendment is reached as an external reference, got {other:?}"),
    }
}

#[test]
fn should_keep_the_time_a_statement_was_made_in_its_provenance() {
    let annotation = title_26_annotation();
    let (from, to) = pair();

    let links = Link::from_annotation(&annotation, &from, &to);
    let link = links.first().expect("at least one link");

    // Every statement has a when. It is core provenance, not a kind payload:
    // a reader that cannot read the payload still needs it to judge the link.
    assert_eq!(
        link.provenance.timestamp,
        Some(annotation.metadata.timestamp),
        "the annotation's timestamp should survive as provenance"
    );
}

#[test]
fn should_carry_kind_specific_facts_in_a_payload_the_core_does_not_read() {
    let annotation = title_26_annotation();
    let (from, to) = pair();

    let links = Link::from_annotation(&annotation, &from, &to);
    let link = links.first().expect("at least one link");

    // The amending action is a legislature concept. It used to be crammed into
    // `Provenance.method` as a `Debug` string, which round-tripped only because
    // the enum happens to carry no data.
    let payload = link
        .payload
        .as_ref()
        .expect("an amendment link carries legislature facts");
    assert_eq!(payload.namespace, "legislature");
    assert!(
        payload.value.get("operation").is_some(),
        "the amending action belongs to the payload, got {:?}",
        payload.value
    );
}

// --- Storage ---

use words_to_data::storage::{InMemoryStorage, LinkReader};

const UNKNOWN_KIND_LINKS: &str = "tests/test_data/processed/unknown_kind_links.json";
const TITLE_9_XML: &str = "tests/test_data/usc/2025-07-18/usc09.xml";

/// Links of a kind this build does not define, over real corpus paths.
fn unknown_kind_links() -> Vec<Link> {
    let json = std::fs::read_to_string(UNKNOWN_KIND_LINKS).expect("the fixture should be readable");
    serde_json::from_str(&json).expect("the fixture should parse as links")
}

fn empty_dataset() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Link storage fixture".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_9_XML, "2025-07-18", None)
        .expect("title 9 should parse");
    dataset
}

/// Round-trip a dataset through SQLite so both backends are exercised.
///
/// The caller must keep the returned directory in scope: dropping it removes the
/// database.
fn through_sqlite(
    dataset: &Dataset<InMemoryStorage>,
) -> (
    tempfile::TempDir,
    Dataset<words_to_data::storage::SqliteStorage>,
) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");
    (dir, sqlite)
}

#[test]
fn should_carry_a_link_of_a_kind_this_build_has_never_seen() {
    let fixture = unknown_kind_links();
    let mut dataset = empty_dataset();
    for link in fixture.clone() {
        dataset.add_link(link).expect("the link should be stored");
    }

    let (_dir, sqlite) = through_sqlite(&dataset);
    for (label, stored) in [
        ("memory", dataset.links_by_namespace("westlaw").unwrap()),
        ("sqlite", sqlite.links_by_namespace("westlaw").unwrap()),
    ] {
        // A reader that does not understand `westlaw.headnote` must still find
        // it, report it, and hand it back byte-identical. Silent loss is the
        // one failure a portable format cannot have.
        assert_eq!(
            stored.len(),
            fixture.len(),
            "{label} should hold both links"
        );
        for expected in &fixture {
            let found = stored
                .iter()
                .find(|l| l.id() == expected.id())
                .unwrap_or_else(|| panic!("{label} lost a link"));
            assert_eq!(
                found, expected,
                "{label} changed a link it does not understand"
            );
        }
    }
}

#[test]
fn should_leave_one_link_when_the_same_statement_is_written_twice() {
    let link = unknown_kind_links().remove(0);
    let mut dataset = empty_dataset();

    dataset.add_link(link.clone()).expect("first write");
    let mut restated = link.clone();
    restated.provenance.source = "westlaw:reimport".to_string();
    dataset.add_link(restated).expect("second write");

    let stored = dataset.links_by_namespace("westlaw").unwrap();
    assert_eq!(
        stored.len(),
        1,
        "restating a fact updates one link rather than growing the table"
    );
    assert_eq!(
        stored[0].provenance.source, "westlaw:reimport",
        "the newer provenance wins when nobody has judged the link"
    );
}

#[test]
fn should_keep_a_human_verdict_when_a_machine_restates_the_fact() {
    let mut confirmed = unknown_kind_links().remove(0);
    confirmed.provenance.source = "human:jesse".to_string();
    confirmed.provenance.verification = VerificationState::HumanConfirmed;

    let mut dataset = empty_dataset();
    dataset.add_link(confirmed.clone()).expect("human write");

    let mut machine = confirmed.clone();
    machine.provenance.source = "model:local".to_string();
    machine.provenance.verification = VerificationState::MachineSuggested;
    dataset.add_link(machine).expect("machine write");

    // Re-running the pipeline must not destroy human review. The loss would be
    // invisible until somebody looked for a confirmation that was gone.
    let stored = dataset.links_by_namespace("westlaw").unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].provenance.verification,
        VerificationState::HumanConfirmed
    );
    assert_eq!(stored[0].provenance.source, "human:jesse");
}

#[test]
fn should_keep_both_links_when_two_amendments_cause_one_change() {
    let base = unknown_kind_links().remove(0);
    let mut second = base.clone();
    second.object = Target::External {
        reference: "westlaw.headnote:9999".to_string(),
        display: "A different headnote".to_string(),
    };

    let mut dataset = empty_dataset();
    dataset.add_link(base).expect("first");
    dataset.add_link(second).expect("second");

    // Same subject, same kind, different object. Several statements about one
    // change are all real, so the table carries no uniqueness on subject+kind.
    assert_eq!(dataset.links_by_namespace("westlaw").unwrap().len(), 2);
}

#[test]
fn should_find_links_by_kind_and_by_the_path_they_are_about() {
    let fixture = unknown_kind_links();
    let mut dataset = empty_dataset();
    for link in fixture.clone() {
        dataset.add_link(link).expect("stored");
    }
    let (_dir, sqlite) = through_sqlite(&dataset);

    let Target::Change { path, .. } = &fixture[0].subject else {
        panic!("the fixture's subject is a change");
    };

    for (label, by_kind, by_path, by_other_kind) in [
        (
            "memory",
            dataset.links_by_kind("westlaw.headnote").unwrap(),
            dataset.links_for_path(path).unwrap(),
            dataset.links_by_kind(LinkKind::AMENDED_BY).unwrap(),
        ),
        (
            "sqlite",
            sqlite.links_by_kind("westlaw.headnote").unwrap(),
            sqlite.links_for_path(path).unwrap(),
            sqlite.links_by_kind(LinkKind::AMENDED_BY).unwrap(),
        ),
    ] {
        assert_eq!(by_kind.len(), 2, "{label} by kind");
        assert_eq!(by_path.len(), 1, "{label} by path");
        assert!(
            by_other_kind.is_empty(),
            "{label} should not answer a kind it holds none of"
        );
    }
}

#[test]
fn should_report_no_annotations_for_a_dataset_of_links_it_does_not_own() {
    let mut dataset = empty_dataset();
    for link in unknown_kind_links() {
        dataset.add_link(link).expect("stored");
    }

    // A `westlaw.headnote` is not a change annotation. The projection is a
    // legislature-shaped view, and must not claim links of another kind.
    let pair = pair();
    assert!(
        dataset
            .get_annotations(&pair.0, &pair.1)
            .unwrap()
            .unwrap_or_default()
            .is_empty()
    );
}

#[test]
fn should_carry_an_unreadable_payload_through_storage_untouched() {
    let mut link = unknown_kind_links().remove(0);
    link.payload = Some(KindPayload {
        namespace: "westlaw".to_string(),
        value: serde_json::json!({
            "nested": {"a": [1, 2, {"b": null}]},
            "unicode": "§ 174 — “quoted”",
        }),
    });
    let expected = link.payload.clone();

    let mut dataset = empty_dataset();
    dataset.add_link(link).expect("stored");
    let (_dir, sqlite) = through_sqlite(&dataset);

    // The core stores it, hands it back unchanged, and never reads it.
    let stored = sqlite.links_by_namespace("westlaw").unwrap();
    assert_eq!(stored[0].payload, expected);
}
