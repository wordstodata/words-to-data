//! The verbatim model reply is kept as evidence (#58).
//!
//! `CONTEXT.md` defines provenance as the source, the method, the evidence, and
//! the verification state. Three of those survived an LLM extraction. The
//! reasoning did too — that part of #58's original claim was withdrawn — but the
//! text *around* the answer never did, so nobody could check that the answer was
//! parsed out of it faithfully.
//!
//! One reply produces many statements, so a reply is stored once under the hash
//! of its own text and referenced, rather than copied onto every link.
//!
//! The fixture is real recorded output. The first sweep to use this mechanism
//! produced 1,237 replies behind 893 links.

use words_to_data::dataset::{Dataset, DatasetMetadata, WorkId};
use words_to_data::link::{Evidence, Link, LinkKind, Provenance, Target, VerificationState};
use words_to_data::storage::{EvidenceReader, InMemoryStorage, LinkReader};

/// A reply a model really emitted, recorded by a `words_to_data` sweep.
///
/// Real output rather than text written to look like it: a reply reconstructed
/// from stored results is well-formed by construction, so it would not exercise
/// the fence a model actually wraps its JSON in.
const REPLIES: &str = "tests/test_data/processed/model_replies.json";

fn recorded_reply() -> String {
    #[derive(serde::Deserialize)]
    struct RecordedReply {
        reply: String,
    }
    let json = std::fs::read_to_string(REPLIES).expect("the fixture should be readable");
    let replies: Vec<RecordedReply> =
        serde_json::from_str(&json).expect("the fixture should parse");
    replies
        .into_iter()
        .next()
        .expect("the fixture should hold a reply")
        .reply
}

fn dataset() -> Dataset<InMemoryStorage> {
    Dataset::new(DatasetMetadata {
        name: "Reply evidence fixture".to_string(),
        ..Default::default()
    })
}

fn a_link(evidence: Option<Evidence>) -> Link {
    Link {
        subject: Target::Change {
            work: WorkId::new("uscode/title_26"),
            path: "uscode/title_26/subtitle_A/chapter_1/section_163".to_string(),
            from_date: "2025-07-18".to_string(),
            to_date: "2025-07-30".to_string(),
        },
        kind: LinkKind::new(LinkKind::AMENDED_BY),
        object: Target::External {
            reference: "legislature.amendment:119-21:abc".to_string(),
            display: "by striking ...".to_string(),
        },
        provenance: Provenance {
            source: "model:local".to_string(),
            method: None,
            verification: VerificationState::MachineSuggested,
            evidence,
            raw_score: Some(0.9),
            timestamp: None,
            corroboration: None,
        },
        payload: None,
    }
}

#[test]
fn should_store_a_reply_once_under_the_hash_of_its_own_text() {
    let mut dataset = dataset();
    let reply = recorded_reply();

    let first = dataset.add_reply(&reply).expect("the reply should store");
    let second = dataset
        .add_reply(&reply)
        .expect("storing it again should be fine");

    // A reply is identified by what it says, like a link and like an amendment.
    // One reply produced many statements, so storing it per statement would
    // claim each statement had its own reply.
    assert_eq!(first, second, "the same text is the same reply");
    assert_eq!(
        dataset.replies().expect("replies should list").len(),
        1,
        "the same reply stored twice is one record"
    );
    assert_eq!(
        dataset.get_reply(&first).expect("readable").as_deref(),
        Some(reply.as_str()),
        "the reply must come back byte-identical, fences and prose included"
    );
}

#[test]
fn should_reach_the_reply_from_a_links_evidence() {
    let mut dataset = dataset();
    let reply_id = dataset.add_reply(&recorded_reply()).expect("stored");

    dataset
        .add_link(a_link(Some(Evidence {
            reasoning: Some("Section reference matches exactly".to_string()),
            reply: Some(reply_id.clone()),
            model: Some("deepseek-v4-pro".to_string()),
            prompt_hash: Some("f00d".to_string()),
        })))
        .expect("the link should store");

    let stored = dataset
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("links should read");
    let evidence = stored[0]
        .provenance
        .evidence
        .as_ref()
        .expect("a machine claim carries evidence");

    // The reasoning explains the claim; the reply is what lets a receiving
    // party check that the reasoning was parsed out of it faithfully.
    assert_eq!(
        evidence.reasoning.as_deref(),
        Some("Section reference matches exactly")
    );
    assert_eq!(evidence.model.as_deref(), Some("deepseek-v4-pro"));
    assert!(evidence.prompt_hash.is_some());
    assert_eq!(
        dataset
            .get_reply(evidence.reply.as_ref().expect("a reply reference"))
            .expect("readable"),
        Some(recorded_reply())
    );
}

#[test]
fn should_keep_a_reply_whose_statement_was_superseded() {
    let mut dataset = dataset();
    let reply_id = dataset.add_reply(&recorded_reply()).expect("stored");

    dataset
        .add_link(a_link(Some(Evidence {
            reasoning: Some("first answer".to_string()),
            reply: Some(reply_id.clone()),
            model: Some("model-a".to_string()),
            prompt_hash: None,
        })))
        .expect("first");

    // Restating the fact replaces the link, so nothing references the old
    // reply any more.
    dataset
        .add_link(a_link(Some(Evidence {
            reasoning: Some("second answer".to_string()),
            reply: None,
            model: Some("model-b".to_string()),
            prompt_hash: None,
        })))
        .expect("second");

    // Evidence is append-only. Deleting a reply because the statement it
    // supported was superseded destroys the trail this exists to create.
    assert_eq!(
        dataset.get_reply(&reply_id).expect("readable"),
        Some(recorded_reply()),
        "an orphaned reply is a record that something was said, not garbage"
    );
}

#[test]
fn should_carry_replies_and_evidence_through_sqlite() {
    let mut memory = dataset();
    let reply_id = memory.add_reply(&recorded_reply()).expect("stored");
    memory
        .add_link(a_link(Some(Evidence {
            reasoning: Some("why".to_string()),
            reply: Some(reply_id.clone()),
            model: Some("deepseek-v4-pro".to_string()),
            prompt_hash: Some("f00d".to_string()),
        })))
        .expect("link stored");

    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("evidence.sqlite");
    memory.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");

    // Evidence that does not travel in the file cannot be checked by the party
    // it was sent to, which is the whole reason for keeping it.
    assert_eq!(
        sqlite.get_reply(&reply_id).expect("readable"),
        Some(recorded_reply())
    );
    let stored = sqlite
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("links should read");
    assert_eq!(
        stored[0].provenance.evidence,
        memory.links_by_kind(LinkKind::AMENDED_BY).unwrap()[0]
            .provenance
            .evidence
    );
}

#[test]
fn should_answer_none_for_a_reply_the_dataset_does_not_hold() {
    let dataset = dataset();

    // A missing reply is a question with an answer. A dataset that never
    // recorded one is different from a dataset whose reply is empty.
    assert_eq!(dataset.get_reply("nosuchhash").expect("readable"), None);
}

#[test]
fn should_say_a_model_produced_an_amendments_word_level_changes() {
    use words_to_data::uslm::bill_parser::parse_bill_amendments;

    let mut dataset = dataset();
    let bill = parse_bill_amendments(
        "119-21",
        "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml",
    )
    .expect("the public law should parse");
    let amendment_id = bill
        .amendments
        .keys()
        .next()
        .expect("the bill should hold amendments")
        .clone();
    dataset.add_bill(bill).expect("the bill should be added");

    // Parsed from the bill, so nothing has been asserted about its changes yet.
    assert!(
        dataset.get_bill("119-21").unwrap().unwrap().amendments[&amendment_id]
            .provenance
            .is_none(),
        "the amending text is a fact from a source; its word-level changes are not"
    );

    let reply_id = dataset.add_reply(&recorded_reply()).expect("stored");
    dataset.set_amendment_provenance(
        &amendment_id,
        Provenance {
            source: "model:local".to_string(),
            method: Some("extract-changes".to_string()),
            verification: VerificationState::MachineSuggested,
            evidence: Some(Evidence {
                reasoning: None,
                reply: Some(reply_id),
                model: Some("local".to_string()),
                prompt_hash: Some("beef".to_string()),
            }),
            raw_score: None,
            timestamp: None,
            corroboration: None,
        },
    );

    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("amendment_provenance.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");

    let stored = sqlite.get_bill("119-21").unwrap().unwrap().amendments[&amendment_id]
        .provenance
        .clone()
        .expect("the provenance should survive the file it travels in");

    // Evidence without a verification state would read as corroboration for
    // what is actually a machine's reading of the amending text.
    assert_eq!(stored.verification, VerificationState::MachineSuggested);
    assert_eq!(stored.source, "model:local");
    assert_eq!(
        sqlite
            .get_reply(stored.evidence.unwrap().reply.as_ref().unwrap())
            .expect("readable"),
        Some(recorded_reply())
    );
}
