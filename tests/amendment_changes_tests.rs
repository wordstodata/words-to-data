//! Recording what a reading found about a bill's amendments (#199).
//!
//! The word-level changes an amendment states are a model's reading, and they
//! are written back onto the amendment the bill parse made. Until now only the
//! in-memory store could take them, because the two methods that wrote them
//! reached into its map of bills. Both stores answer the same call now.
//!
//! The material is real: HR 1 of the 119th Congress, parsed from the corpus,
//! and the changes are a recorded model reply from a production run.

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::legislature::{AmendmentChanges, BillDiff};
use words_to_data::link::{
    Link, LinkKind, Provenance, Target, VerificationState, amendment_reference_parts,
};
use words_to_data::method::Method;
use words_to_data::storage::{LinkReader, SqliteStorage};
use words_to_data::uslm::bill_parser::{Bill, parse_bill_amendments};

/// A real public law from the test corpus: HR 1 of the 119th Congress.
const BILL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-21";

/// One real extraction result, as a production run recorded it.
const REAL_EXTRACTION: &str = r#"<response>[{"added":["of—\"(A)"],"removed":["of"]},{"added":[";"],"removed":["."]}]</response>"#;

fn real_bill() -> Bill {
    parse_bill_amendments(BILL_ID, BILL_XML).expect("the public law should parse")
}

fn real_changes() -> Vec<BillDiff> {
    words_to_data::llm::parse_changes(REAL_EXTRACTION).expect("the real extraction should parse")
}

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Amendment changes".to_string(),
        description: "HR 1, for recording word-level changes".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    }
}

/// The id of the amendment this file records against, taken in sorted order so
/// every test in the file speaks about the same one.
fn first_amendment_id(bill: &Bill) -> String {
    let mut ids: Vec<String> = bill.amendments.keys().cloned().collect();
    ids.sort();
    ids.into_iter().next().expect("the bill holds amendments")
}

fn reading(amendment_id: &str) -> AmendmentChanges {
    AmendmentChanges {
        bill_id: BILL_ID.to_string(),
        amendment_id: amendment_id.to_string(),
        changes: real_changes(),
        provenance: Some(Provenance {
            source: "model:local".to_string(),
            method: Some(Method::new("extract-changes", 1)),
            verification: VerificationState::MachineSuggested,
            evidence: None,
            raw_score: None,
            timestamp: None,
            corroboration: None,
        }),
    }
}

/// A database holding the real bill, in this test's own directory.
fn database(name: &str) -> Dataset<SqliteStorage> {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let mut memory = Dataset::new(metadata());
    memory.add_bill(real_bill()).expect("the bill is added");
    memory.save_to_sqlite(&path).expect("the fixture saves");
    Dataset::open_sqlite(&path).expect("the fixture opens")
}

#[test]
fn should_record_changes_and_provenance_on_one_amendment_when_the_dataset_is_a_database() {
    let bill = real_bill();
    let amendment_id = first_amendment_id(&bill);
    let mut dataset = database("amendment_changes_db");

    let written = dataset
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording the reading should work");

    assert_eq!(written, 1, "one amendment was named, so one was written");

    let stored = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should still be there");
    let amendment = &stored.amendments[&amendment_id];
    assert_eq!(
        amendment.changes,
        real_changes(),
        "the database should hold the changes the reading found"
    );
    assert_eq!(
        amendment.provenance.as_ref().map(|p| p.source.as_str()),
        Some("model:local"),
        "the database should hold where the changes came from"
    );
}

#[test]
fn should_record_changes_and_provenance_on_one_amendment_when_the_dataset_is_a_w2d_file() {
    let bill = real_bill();
    let amendment_id = first_amendment_id(&bill);
    let mut dataset = Dataset::new(metadata());
    dataset.add_bill(bill).expect("the bill is added");

    let written = dataset
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording the reading should work");

    assert_eq!(written, 1, "one amendment was named, so one was written");

    let stored = dataset
        .get_bill(BILL_ID)
        .expect("reading the bill should work")
        .expect("the bill should still be there");
    let amendment = &stored.amendments[&amendment_id];
    assert_eq!(
        amendment.changes,
        real_changes(),
        "the file should hold the changes the reading found"
    );
    assert_eq!(
        amendment.provenance.as_ref().map(|p| p.source.as_str()),
        Some("model:local"),
        "the file should hold where the changes came from"
    );
}

/// The two forms are given the same reading and must hold the same thing.
#[test]
fn should_hold_the_same_changes_in_both_forms_when_the_same_reading_is_recorded() {
    let bill = real_bill();
    let amendment_id = first_amendment_id(&bill);

    let mut memory = Dataset::new(metadata());
    memory.add_bill(bill).expect("the bill is added");
    memory
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording into the file should work");

    let mut database = database("amendment_changes_both");
    database
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording into the database should work");

    let from_memory = memory.get_bill(BILL_ID).unwrap().unwrap();
    let from_database = database.get_bill(BILL_ID).unwrap().unwrap();
    assert_eq!(
        from_memory.amendments[&amendment_id], from_database.amendments[&amendment_id],
        "both forms should hold the same amendment after the same reading"
    );
}

/// An amendment this dataset does not hold is skipped rather than invented, and
/// the count says so.
#[test]
fn should_skip_an_amendment_the_dataset_does_not_hold_when_recording() {
    let mut dataset = database("amendment_changes_unknown");

    let written = dataset
        .update_amendments(&[reading("an id no bill in this dataset carries")])
        .expect("recording should work");

    assert_eq!(
        written, 0,
        "nothing was written, and the count should say so"
    );
}

/// Format is not part of this: a reading recorded against a database and then
/// written out as a W2D file reads back the same.
#[test]
fn should_carry_the_recorded_changes_through_a_w2d_file_when_the_database_is_written_out() {
    let bill = real_bill();
    let amendment_id = first_amendment_id(&bill);
    let mut dataset = database("amendment_changes_roundtrip");
    dataset
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording should work");

    let path = format!(
        "{}/amendment_changes_roundtrip.json",
        env!("CARGO_TARGET_TMPDIR")
    );
    dataset
        .to_memory()
        .expect("the database should read into memory")
        .save(&path, Format::Compact)
        .expect("the file should save");

    let reloaded = Dataset::load(&path, Format::Compact).expect("the file should load");
    let amendment = &reloaded.get_bill(BILL_ID).unwrap().unwrap().amendments[&amendment_id];
    assert_eq!(amendment.changes, real_changes());
}

// --- An amendment's identity does not move ---
//
// An amendment is identified by `sha256("{bill_id}:{amending_text}")`, and a
// `legislature.amended_by` link names it by that hash (`docs/adr/0009`). If
// recording a change moved the hash, every such link would point at nothing.
// These two tests resolve a real link rather than assert the rule.

/// The real matching run this corpus carries, which names real amendments of
/// HR 1 by their content hash.
const ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";
const TITLE_26: &str = "uscode/title_26";

/// A real `legislature.amended_by` link that names an amendment this dataset
/// holds, and that amendment's id.
///
/// The annotation is a real one from the recorded matching run. It is pointed
/// at the amendment the fixture holds, because the ids the run recorded were
/// minted by an older parse of the same bill and no longer name an amendment of
/// it. A link has to name something the dataset holds, or it cannot be shown to
/// stop resolving either.
fn real_link_naming_an_amendment() -> (Link, String) {
    let json = std::fs::read_to_string(ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");
    let mut annotation = annotations
        .into_iter()
        .find(|a| a.source_bill.bill_id == BILL_ID)
        .expect("the fixture should name an amendment of this bill");
    let amendment_id = first_amendment_id(&real_bill());
    annotation.source_bill.amendment_id = amendment_id.clone();

    let from = ExpressionId::new(WorkId::new(TITLE_26), "2025-07-18");
    let to = ExpressionId::new(WorkId::new(TITLE_26), "2025-07-30");
    let link = Link::from_annotation(&annotation, &from, &to)
        .into_iter()
        .next()
        .expect("the annotation should give a link");
    (link, amendment_id)
}

/// Follow a link's object back to the amendment it names, the way a reader
/// does: take the reference apart, then ask the dataset for that amendment.
fn resolve<S>(dataset: &Dataset<S>, link: &Link) -> Option<String>
where
    S: words_to_data::storage::Storage + words_to_data::storage::LegislatureReader,
{
    let Target::External { reference, .. } = &link.object else {
        return None;
    };
    let (bill_id, amendment_id) = amendment_reference_parts(reference)?;
    let bill = dataset.get_bill(bill_id).ok()??;
    Some(bill.amendments.get(amendment_id)?.id.clone())
}

#[test]
fn should_still_resolve_a_link_that_names_the_amendment_when_changes_are_recorded_in_a_database() {
    let (link, amendment_id) = real_link_naming_an_amendment();
    let mut dataset = database("amendment_changes_link_db");
    dataset.add_link(link).expect("the link is stored");

    dataset
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording the reading should work");

    // Read the link back out of the dataset rather than reuse the one above: a
    // link that stopped resolving because it was never stored proves nothing.
    let stored = dataset
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("reading links should work")
        .into_iter()
        .next()
        .expect("the dataset should still hold the link");

    assert_eq!(
        resolve(&dataset, &stored),
        Some(amendment_id),
        "the link should still find the amendment it names"
    );
}

#[test]
fn should_still_resolve_a_link_that_names_the_amendment_when_changes_are_recorded_in_a_w2d_file() {
    let (link, amendment_id) = real_link_naming_an_amendment();
    let mut dataset = Dataset::new(metadata());
    dataset.add_bill(real_bill()).expect("the bill is added");
    dataset.add_link(link).expect("the link is stored");

    dataset
        .update_amendments(&[reading(&amendment_id)])
        .expect("recording the reading should work");

    let path = format!(
        "{}/amendment_changes_link.json",
        env!("CARGO_TARGET_TMPDIR")
    );
    dataset
        .save(&path, Format::Compact)
        .expect("it should save");
    let reloaded = Dataset::load(&path, Format::Compact).expect("it should load");

    let stored = reloaded
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("reading links should work")
        .into_iter()
        .next()
        .expect("the file should still hold the link");

    assert_eq!(
        resolve(&reloaded, &stored),
        Some(amendment_id.clone()),
        "the link should still find the amendment it names"
    );

    // And the hash is the one a fresh parse of the same bill mints, so the
    // identity is the source's, not something this recording produced.
    assert!(
        real_bill().amendments.contains_key(&amendment_id),
        "a fresh parse of the bill should mint the same amendment id"
    );
}
