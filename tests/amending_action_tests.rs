//! The amending action vocabulary, against the publisher's schema (#156).
//!
//! `AmendingAction` says what a bill does to existing law, and the words come
//! from the publisher: `AmendingActionTypeEnum` in `uslm-2.0.17.xsd:610`, which is
//! committed at the root of this repo, lists twelve and no more. The literals
//! below are that list, copied from the schema in the order the schema writes
//! them. They are a real source, not invented data: a `type` attribute in a
//! conforming bill holds one of them.
//!
//! Only six of the twelve appear in the committed markup — insert 1282, delete
//! 1198, amend 1152, add 388, redesignate 114, repeal 26, counted over every
//! `amendingAction` in `tests/test_data` — so the other six are tested through
//! their schema literal rather than through a bill. Inventing a bill fragment to
//! carry one would make up a source, which `CLAUDE.md` forbids.
//!
//! That markup is Public Law 119-21 and nothing else: the corpus holds it twice,
//! raw and as the congress client cached it, so "the five cached public laws" the
//! issue speaks of is one law in two copies.

use std::collections::BTreeMap;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::legislature::AmendingAction;
use words_to_data::uslm::bill_parser::parse_bill_amendments_from_str_with_report;

/// Public Law 119-21, as `words_to_data` downloaded it.
const PUBLIC_LAW: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

/// Annotations a real `match-amendments` sweep recorded and stored.
const STORED: &str = "tests/test_data/processed/annotations.json";

/// The publisher's schema, as committed at the root of this repo.
const SCHEMA: &str = "uslm-2.0.17.xsd";

/// Every value of `AmendingActionTypeEnum`, in the schema's own order.
const SCHEMA_VALUES: [(&str, AmendingAction); 12] = [
    ("enact", AmendingAction::Enact),
    ("add", AmendingAction::Add),
    ("amend", AmendingAction::Amend),
    ("substitute", AmendingAction::Substitute),
    ("redesignate", AmendingAction::Redesignate),
    ("repeal", AmendingAction::Repeal),
    ("repealAndReserve", AmendingAction::RepealAndReserve),
    ("insert", AmendingAction::Insert),
    ("delete", AmendingAction::Delete),
    ("conform", AmendingAction::Conform),
    ("noChange", AmendingAction::NoChange),
    ("unknown", AmendingAction::Unknown),
];

/// Every `xsd:enumeration` value of `AmendingActionTypeEnum`, read from the schema.
///
/// The list above is a transcription, and a transcription can drift from what it
/// copied. This reads the committed schema instead, so the two can be compared.
fn values_in_the_schema() -> Vec<String> {
    let xsd = std::fs::read_to_string(SCHEMA).expect("the schema should be committed");
    let opens = "<xsd:simpleType name=\"AmendingActionTypeEnum\">";
    let from = xsd.find(opens).expect("the schema should define the type") + opens.len();
    let to = from
        + xsd[from..]
            .find("</xsd:simpleType>")
            .expect("the type should close");

    xsd[from..to]
        .split("<xsd:enumeration value=\"")
        .skip(1)
        .map(|rest| {
            rest.split_once('"')
                .expect("an enumeration value should close its quote")
                .0
                .to_string()
        })
        .collect()
}

#[test]
fn should_cover_the_schema_when_the_expected_values_are_read_from_it() {
    let in_the_schema = values_in_the_schema();

    let transcribed: Vec<String> = SCHEMA_VALUES
        .iter()
        .map(|(literal, _)| (*literal).to_string())
        .collect();
    // The same values, in the same order. A publisher that adds a thirteenth
    // action fails here, which is the one place that can notice.
    assert_eq!(in_the_schema, transcribed);
}

#[test]
fn should_read_the_action_when_the_schema_defines_the_value() {
    for (literal, expected) in SCHEMA_VALUES {
        let read: AmendingAction = literal
            .parse()
            .unwrap_or_else(|e| panic!("the schema writes {literal:?}: {e}"));
        assert_eq!(read, expected, "{literal:?} must read as {expected:?}");
    }
}

#[test]
fn should_refuse_the_word_when_the_schema_cannot_produce_it() {
    // `move` appears nowhere in `uslm-2.0.17.xsd`, and the schema's words for
    // striking and for striking and inserting are `delete` and `insert`. All
    // three were ours, so no `amendingAction/@type` can carry one, and reading
    // one here would invent a fact about the bill.
    for invented in ["move", "strike", "strikeandinsert", "strike_and_insert"] {
        let read = invented.parse::<AmendingAction>();
        assert!(
            read.is_err(),
            "{invented:?} is not a value of AmendingActionTypeEnum, got {read:?}"
        );
    }
}

#[test]
fn should_report_nothing_when_every_action_a_real_bill_writes_is_a_schema_value() {
    let xml = std::fs::read_to_string(PUBLIC_LAW).expect("the download should be readable");

    let (bill, report) = parse_bill_amendments_from_str_with_report("119-21", &xml)
        .expect("the public law should parse");

    assert!(
        !bill.amendments.is_empty(),
        "a real public law carries amendments"
    );
    // The honest answer to "this bill writes no action we cannot read". It is
    // only worth anything because the report can say otherwise.
    assert!(
        report.is_empty(),
        "119-21 writes insert, delete, amend, add, redesignate and repeal only, got {report:?}"
    );
    assert!(report.summary().is_empty());
    for amendment in bill.amendments.values() {
        for action in &amendment.action_types {
            assert!(
                SCHEMA_VALUES.iter().any(|(_, value)| value == action),
                "{action:?} is not a value the publisher's schema defines"
            );
        }
    }
}

#[test]
fn should_keep_the_meaning_when_a_stored_annotation_holds_the_drafters_word() {
    // A real sweep stored 753 annotations, and 150 of them hold a word the
    // publisher's schema does not define: `strike_and_insert` 110 times and
    // `strike` 40 times. Those records were written before #156 and they are
    // still records. Reading one as `amend` would put a different fact in the
    // dataset, and refusing the file would make every dataset of that age
    // unreadable, so the old word reads as the schema's word for the same act.
    let json = std::fs::read_to_string(STORED).expect("the recorded sweep should be readable");

    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("a stored annotation must still read");

    assert_eq!(annotations.len(), 753, "the whole sweep must read");
    let mut counted: BTreeMap<String, usize> = BTreeMap::new();
    for annotation in &annotations {
        *counted
            .entry(format!("{:?}", annotation.operation))
            .or_default() += 1;
    }
    // `strike_and_insert` 110 -> Substitute, and nothing else in the sweep said
    // substitute.
    assert_eq!(counted.get("Substitute"), Some(&110), "got {counted:?}");
    // `strike` 40 joins the 13 written `delete`.
    assert_eq!(counted.get("Delete"), Some(&53), "got {counted:?}");
    // The count the sweep really wrote. A word that had degraded to the default
    // would show up here as a surplus.
    assert_eq!(counted.get("Amend"), Some(&357), "got {counted:?}");
}
