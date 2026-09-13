//! The reply parsers, against replies models really emitted (#58).
//!
//! `tests/test_data/processed/model_replies.json` holds real output recorded by
//! a `words_to_data` sweep — not text written to look like model output. That
//! distinction is the whole reason #58 exists: a reply reconstructed from
//! stored results is well-formed by construction, so it cannot prove the parser
//! survives what a model actually sends.

use serde::Deserialize;
use words_to_data::legislature::AmendingAction;
use words_to_data::llm::{parse_annotations, parse_changes};

const REPLIES: &str = "tests/test_data/processed/model_replies.json";

#[derive(Deserialize)]
struct RecordedReply {
    name: String,
    command: String,
    reply: String,
}

fn recorded() -> Vec<RecordedReply> {
    let json = std::fs::read_to_string(REPLIES).expect("the fixture should be readable");
    serde_json::from_str(&json).expect("the fixture should parse")
}

fn for_command(command: &str) -> Vec<RecordedReply> {
    recorded()
        .into_iter()
        .filter(|r| r.command == command)
        .collect()
}

#[test]
fn should_parse_every_recorded_match_amendments_reply() {
    let replies = for_command("match-amendments");
    assert!(!replies.is_empty(), "the fixture should hold some");

    for recorded in replies {
        parse_annotations(&recorded.reply)
            .unwrap_or_else(|e| panic!("failed on {:?}: {e}", recorded.name));
    }
}

#[test]
fn should_read_through_the_fence_a_model_wraps_its_json_in() {
    let replies = for_command("match-amendments");

    // Fencing is the common case, not an edge case: 563 of the replies in the
    // first recorded sweep were fenced.
    let fenced: Vec<&RecordedReply> = replies.iter().filter(|r| r.reply.contains("```")).collect();
    assert!(!fenced.is_empty(), "the fixture should hold a fenced reply");

    for recorded in fenced {
        assert!(
            recorded.reply.trim_start().starts_with("```"),
            "{}: the fence is the first thing in the reply",
            recorded.name
        );
        parse_annotations(&recorded.reply)
            .unwrap_or_else(|e| panic!("the fence should be stripped, {}: {e}", recorded.name));
    }
}

#[test]
fn should_ignore_fields_a_model_adds_that_we_never_asked_for() {
    let no_match = for_command("match-amendments")
        .into_iter()
        .find(|r| r.reply.contains("no_match_reasoning"))
        .expect("a reply where the model explained why nothing matched");

    // The model volunteers `no_match_reasoning` beside an empty list. A reply
    // is not wrong for saying more than we asked, and refusing it would throw
    // away a valid answer.
    let annotations =
        parse_annotations(&no_match.reply).expect("an unknown field must not fail the parse");
    assert!(
        annotations.is_empty(),
        "no candidate matched, so there is nothing to record"
    );
}

#[test]
fn should_read_several_annotations_from_one_reply() {
    let many = for_command("match-amendments")
        .into_iter()
        .find(|r| r.reply.matches("\"candidate_index\"").count() >= 3)
        .expect("a reply proposing several annotations");

    let annotations = parse_annotations(&many.reply).expect("should parse");
    assert!(
        annotations.len() >= 3,
        "one reply can answer for several candidates, got {}",
        annotations.len()
    );
    // Every operation the model names must survive; one that cannot be read
    // silently becomes `Amend`, which is a quiet lie about what the law did.
    //
    // A model answers in the drafter's words, not the publisher's, so the reading
    // is `from_prose` rather than `parse`. `parse` is the schema's vocabulary and
    // declines `strikeandinsert` on purpose (#156).
    for annotation in &annotations {
        if let Some(operation) = &annotation.operation {
            assert!(
                AmendingAction::from_prose(operation).is_ok(),
                "the model emitted an operation we cannot read: {operation:?}"
            );
        }
    }
}

#[test]
fn should_map_the_drafters_word_onto_the_schema_when_a_model_answers_in_prose() {
    // Real recorded output: models answered `strikeandinsert` twice in the first
    // sweep, and `tests/test_data/processed/annotations.json` holds 110
    // `strike_and_insert` and 40 `strike` from that pipeline. None of the three
    // is a value of `AmendingActionTypeEnum`, so the word has to land on the
    // publisher's word for the same act.
    let operations: Vec<String> = for_command("match-amendments")
        .iter()
        .filter_map(|recorded| parse_annotations(&recorded.reply).ok())
        .flatten()
        .filter_map(|annotation| annotation.operation)
        .collect();
    assert!(
        operations.iter().any(|op| op == "strikeandinsert"),
        "the fixture should hold a prose word, got {operations:?}"
    );

    // Striking text is the schema's `delete`, and striking text and putting other
    // text in its place is its `substitute`: "replaces an existing provision".
    assert_eq!(
        AmendingAction::from_prose("strike").expect("a drafter's word for delete"),
        AmendingAction::Delete
    );
    for written in ["strikeandinsert", "strike_and_insert"] {
        assert_eq!(
            AmendingAction::from_prose(written).expect("a drafter's word for substitute"),
            AmendingAction::Substitute,
            "{written:?} must read as the schema's word"
        );
    }

    // The schema's own words still read, because a model is asked for an action
    // and often names one.
    assert_eq!(
        AmendingAction::from_prose("redesignate").expect("the schema's word"),
        AmendingAction::Redesignate
    );

    // `move` is refused rather than mapped. The schema has no action for a
    // relocation, and `redesignate` is a different fact: it renumbers a provision
    // that stays where it is. Mapping the two together would record something the
    // model did not say.
    assert!(
        AmendingAction::from_prose("move").is_err(),
        "no word of the schema means relocation"
    );
}

#[test]
fn should_parse_every_recorded_extract_changes_reply() {
    let replies = for_command("extract-changes");
    assert!(!replies.is_empty(), "the fixture should hold some");

    for recorded in replies {
        let changes = parse_changes(&recorded.reply)
            .unwrap_or_else(|e| panic!("failed on {:?}: {e}", recorded.name));
        // An empty extraction is a real answer: the model found no word-level
        // change in that amendment.
        if recorded.name.contains("no word-level change") {
            assert!(changes.is_empty());
        } else {
            assert!(!changes.is_empty(), "{}", recorded.name);
        }
    }
}

#[test]
fn should_say_what_is_wrong_when_a_reply_cannot_be_read() {
    // A reply that never arrived, or arrived truncated, must fail with a reason
    // rather than a panic — this is the path that used to discard the evidence.
    assert!(parse_annotations("").is_err());
    assert!(parse_annotations("I cannot help with that request.").is_err());
    assert!(parse_changes("<response>[{\"added\":").is_err());
    assert!(
        parse_changes("no tags here")
            .unwrap_err()
            .contains("<response>")
    );
}
