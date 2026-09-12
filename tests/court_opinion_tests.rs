//! A court opinion stores honestly, and nothing about it says US Code.
//!
//! Before #129 an opinion could only be stored by declaring a `DocumentType` of
//! US Code or bill, and by taking an `ElementType` out of USLM's legislative
//! vocabulary. Both are false statements about what the document is, written into
//! a file and sent to another party. This is the test that they are gone.
//!
//! The record is real and cached, not written by hand: *Obergefell v. Hodges*,
//! CourtListener opinion 2812209, with its cluster beside it. **The filing date is
//! on the cluster.** The opinion's own `date_created` is when CourtListener
//! ingested the record, and using it would date the expression to the day a third
//! party scraped it.

use serde_json::Value;
use words_to_data::dataset::{Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId};
use words_to_data::document::{ClassPayload, DocumentNode, NodeData, NodeType, text_method};
use words_to_data::inspect;
use words_to_data::judicial::OpinionFacts;
use words_to_data::link::{Provenance, VerificationState};
use words_to_data::uslm::UslmFacts;

const OPINION_JSON: &str = "tests/test_data/courtlistener/opinion_2812209.json";
const CLUSTER_JSON: &str = "tests/test_data/courtlistener/cluster_2812209.json";

/// The work an opinion is an expression of: `judicial/opinion_2812209`.
///
/// A structural path on the same rule as `uscode/title_26`: a class segment, then
/// `<kind>_<number>`. The number is the publisher's id, which is the only stable
/// name the source gives.
const WORK: &str = "judicial/opinion_2812209";

/// A sentence from the middle of the opinion, used to prove that search reaches
/// the text rather than the heading.
const FROM_THE_OPINION: &str = "The Constitution promises liberty to all within its reach";

/// The two cached records, as the API returned them.
fn cached_records() -> (Value, Value) {
    let opinion: Value = serde_json::from_str(
        &std::fs::read_to_string(OPINION_JSON).expect("the cached opinion should read"),
    )
    .expect("the cached opinion should be JSON");
    let cluster: Value = serde_json::from_str(
        &std::fs::read_to_string(CLUSTER_JSON).expect("the cached cluster should read"),
    )
    .expect("the cached cluster should be JSON");

    // The opinion request is a list query, so the record is the single result.
    let opinion = opinion["results"][0].clone();
    (opinion, cluster)
}

/// How the reporters print this opinion: `576 U.S. 644`, `135 S. Ct. 2584`.
fn parallel_citations(cluster: &Value) -> Vec<String> {
    cluster["citations"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|citation| {
            format!(
                "{} {} {}",
                citation["volume"].as_str().unwrap_or_default(),
                citation["reporter"].as_str().unwrap_or_default(),
                citation["page"].as_str().unwrap_or_default(),
            )
        })
        .collect()
}

/// Build the expression this dataset would store for the cached opinion.
///
/// One node, no children. The structure is in the text and nothing has parsed it:
/// `xml_harvard` carries `<opinion type="majority">` and is deliberately passed
/// over here (#53).
fn obergefell() -> Expression {
    let (opinion, cluster) = cached_records();

    // The filing date, from the cluster. Not `opinion["date_created"]`.
    let date_filed = cluster["date_filed"]
        .as_str()
        .expect("the cluster should carry a filing date");
    let date = words_to_data::date::date_str_to_date(date_filed).expect("it should be a date");

    let facts = OpinionFacts {
        case_name: cluster["case_name"]
            .as_str()
            .expect("the cluster should name the case")
            .to_string(),
        opinion_type: opinion["type"].as_str().map(str::to_string),
        author: opinion["author_str"]
            .as_str()
            .filter(|author| !author.is_empty())
            .map(str::to_string),
        precedential_status: cluster["precedential_status"].as_str().map(str::to_string),
        citations: parallel_citations(&cluster),
        source: cluster["source"].as_str().map(str::to_string),
        sha1: opinion["sha1"].as_str().map(str::to_string),
        page_count: opinion["page_count"].as_u64().map(|pages| pages as u32),
    };

    // How the text was obtained is core provenance, not a judicial fact: a reader
    // deciding whether to trust a passage must not have to open a payload to
    // learn that a machine read it off a scan.
    let extracted_by_ocr = opinion["extracted_by_ocr"]
        .as_bool()
        .expect("the record should say whether the text was OCR'd");
    let provenance = Provenance {
        source: "courtlistener".to_string(),
        method: Some(
            match extracted_by_ocr {
                true => text_method::OCR,
                false => text_method::TEXT_LAYER,
            }
            .to_string(),
        ),
        verification: VerificationState::Asserted,
        evidence: None,
        raw_score: None,
        timestamp: None,
        corroboration: None,
    };

    let data = NodeData {
        // The case name is what a reader sees when the opinion is listed, so it
        // is the node's heading. It stays in the payload too, under the name a
        // judicial reader asks for it by.
        heading: Some(facts.case_name.clone().into()),
        content: Some(
            opinion["plain_text"]
                .as_str()
                .expect("the record should carry the opinion text")
                .into(),
        ),
        ..NodeData::new(WORK, NodeType::new(NodeType::OPINION), date)
    }
    .with_provenance(provenance)
    .with_payload(facts.to_payload().expect("the facts should serialize"));

    Expression {
        id: ExpressionId::new(WorkId::new(WORK), date_filed),
        label: None,
        root: DocumentNode::leaf(data),
    }
}

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "One court opinion".to_string(),
        description: "Obergefell v. Hodges, from CourtListener".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec!["https://www.courtlistener.com/api/rest/v4/".to_string()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

fn dataset_holding_the_opinion() -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());
    dataset
        .add_expression(obergefell())
        .expect("an opinion should be storable");
    dataset
}

/// Every node of a tree, root first.
fn nodes(node: &DocumentNode, into: &mut Vec<NodeData>) {
    into.push(node.data.clone());
    for child in &node.children {
        nodes(child, into);
    }
}

#[test]
fn should_store_the_opinion_as_a_single_node_dated_from_the_cluster() {
    let dataset = dataset_holding_the_opinion();

    let id = ExpressionId::new(WorkId::new(WORK), "2015-06-26");
    let stored = dataset
        .get_expression(&id)
        .expect("the dataset should answer")
        .expect("the opinion should be held");

    assert_eq!(stored.root.children.len(), 0, "an opinion is one node");
    assert_eq!(stored.root.data.node_type.as_str(), "judicial.opinion");
    assert_eq!(stored.root.data.date.to_string(), "2015-06-26");
    // Characters, not bytes: 206,987 is the figure the source reports, and the
    // opinion is full of typographic quotes and section marks.
    assert_eq!(
        stored
            .root
            .data
            .content
            .as_deref()
            .map(|text| text.chars().count()),
        Some(206_987),
        "the opinion text should arrive whole"
    );
    assert!(
        stored
            .root
            .data
            .content
            .as_deref()
            .expect("the opinion should carry text")
            .contains(FROM_THE_OPINION)
    );
}

/// The point of the whole change. Asserted against the stored record rather than
/// read by eye: everything but the opinion's own text is serialized and searched
/// for a claim about the US Code or a bill.
///
/// The text is excluded because the opinion really does name a `Bill Haslam`, and
/// a test that failed on the words of the law would be testing the wrong thing.
#[test]
fn should_claim_nothing_about_the_us_code_or_a_bill_when_an_opinion_is_stored() {
    let expression = obergefell();

    let mut all = Vec::new();
    nodes(&expression.root, &mut all);

    for mut data in all {
        // The node's own words are the document, not a claim about it.
        data.heading = None;
        data.chapeau = None;
        data.proviso = None;
        data.content = None;
        data.continuation = None;

        let record = serde_json::to_string(&data).expect("a node should serialize");
        for claim in ["uscode", "us_code", "usc_type", "public_law", "bill"] {
            assert!(
                !record.contains(claim),
                "a stored opinion must not say `{claim}`: {record}"
            );
        }
    }
}

/// The same statement from the other side: the node carries no USLM facts at all.
#[test]
fn should_carry_no_uslm_facts_when_the_node_is_an_opinion() {
    let expression = obergefell();

    assert!(
        UslmFacts::of(&expression.root.data).is_none(),
        "a USLM reader has nothing to say about a court opinion"
    );
    assert_eq!(
        expression.root.data.node_type.namespace(),
        NodeType::JUDICIAL
    );
}

/// `extracted_by_ocr` says how the text was obtained, so it is provenance rather
/// than a judicial fact. A reader gets it without opening any payload.
#[test]
fn should_reach_how_the_text_was_obtained_without_opening_a_payload() {
    let expression = obergefell();

    let provenance = expression
        .root
        .data
        .provenance
        .as_ref()
        .expect("an opinion records where its text came from");

    assert_eq!(provenance.source, "courtlistener");
    assert_eq!(
        provenance.method.as_deref(),
        Some(text_method::TEXT_LAYER),
        "this record says extracted_by_ocr is false, so the text came from a text layer"
    );
    assert_eq!(provenance.verification, VerificationState::Asserted);

    // And the payload says nothing about it, so there is one place to look.
    let payload = expression
        .root
        .data
        .payload
        .as_ref()
        .expect("an opinion carries judicial facts");
    assert!(!payload.value.contains("ocr"));
}

/// Every judicial fact the source supplies must survive both on-disk forms. A
/// payload the core never reads is only useful if it comes back unchanged.
#[test]
fn should_keep_every_judicial_fact_through_the_sqlite_backend() {
    let directory = tempfile::tempdir().expect("a test directory");
    let path = directory.path().join("opinion.sqlite");

    dataset_holding_the_opinion()
        .save_to_sqlite(&path)
        .expect("the dataset should save");

    let reopened = Dataset::open_sqlite(&path).expect("the dataset should reopen");
    assert_judicial_facts_survived(&reopened);
}

#[test]
fn should_keep_every_judicial_fact_through_the_compact_file() {
    let directory = tempfile::tempdir().expect("a test directory");
    let path = directory.path().join("opinion.w2d");
    let path = path.to_str().expect("a utf-8 path");

    dataset_holding_the_opinion()
        .save(path, Format::Compact)
        .expect("the dataset should save");

    let reopened = Dataset::load(path, Format::Compact).expect("the dataset should reopen");
    assert_judicial_facts_survived(&reopened);
}

/// What a round trip has to preserve, whichever form carried it.
fn assert_judicial_facts_survived<S: words_to_data::storage::Storage>(dataset: &Dataset<S>) {
    let id = ExpressionId::new(WorkId::new(WORK), "2015-06-26");
    let stored = dataset
        .get_expression(&id)
        .expect("the dataset should answer")
        .expect("the opinion should still be held");

    let facts = OpinionFacts::of(&stored.root.data).expect("the judicial facts should come back");
    assert_eq!(facts.case_name, "Obergefell v. Hodges");
    assert_eq!(facts.opinion_type.as_deref(), Some("010combined"));
    assert_eq!(facts.author.as_deref(), Some("Kennedy"));
    assert_eq!(facts.precedential_status.as_deref(), Some("Published"));
    assert_eq!(facts.source.as_deref(), Some("CU"));
    assert_eq!(facts.page_count, Some(103));
    assert!(
        facts.citations.contains(&"576 U.S. 644".to_string()),
        "the reporters' citations should survive, got {:?}",
        facts.citations
    );

    // Provenance is core, so it travels beside the payload rather than in it.
    assert_eq!(
        stored
            .root
            .data
            .provenance
            .as_ref()
            .and_then(|p| p.method.clone())
            .as_deref(),
        Some(text_method::TEXT_LAYER)
    );
    assert_eq!(stored.root.data.node_type.as_str(), "judicial.opinion");
}

#[test]
fn should_report_the_opinion_through_search_expressions_and_info() {
    let dataset = dataset_holding_the_opinion();

    let hits = inspect::search(&dataset, FROM_THE_OPINION).expect("the search should run");
    assert!(
        hits.iter().any(|hit| hit.path == WORK),
        "search should reach the opinion's text, got {hits:?}"
    );

    let listed = inspect::expressions(&dataset, None).expect("the listing should run");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "judicial/opinion_2812209@2015-06-26");
    assert_eq!(listed[0].element_count, 1, "one node, and it is counted");

    let info = inspect::info(&dataset).expect("info should run");
    assert_eq!(info.work_count, 1);
    assert_eq!(info.expression_count, 1);
    assert_eq!(info.bill_count, 0, "a judicial dataset holds no bills");
}

/// ADR 0002's rule for links, applied to nodes: a reader that does not know a
/// class must carry what it cannot interpret rather than drop it.
///
/// `westlaw.headnote` is a class this build has never heard of, and nothing in it
/// is registered anywhere. Both on-disk forms must hand it back exactly.
#[test]
fn should_preserve_a_node_type_and_payload_this_build_does_not_know() {
    let date = words_to_data::date::date_str_to_date("2015-06-26").expect("a date");
    let unknown_facts = r#"{"topic":"Marriage","reporter_note":"held unconstitutional"}"#;

    let data = NodeData {
        content: Some("A headnote written by somebody else.".into()),
        ..NodeData::new(
            "westlaw/headnote_1",
            NodeType::new("westlaw.headnote"),
            date,
        )
    }
    .with_payload(ClassPayload {
        namespace: "westlaw".into(),
        value: unknown_facts.into(),
    });

    let mut dataset = Dataset::new(metadata());
    dataset
        .add_expression(Expression {
            id: ExpressionId::new(WorkId::new("westlaw/headnote_1"), "2015-06-26"),
            label: None,
            root: DocumentNode::leaf(data),
        })
        .expect("an unknown class should still be storable");

    let directory = tempfile::tempdir().expect("a test directory");
    let sqlite = directory.path().join("unknown.sqlite");
    let compact = directory.path().join("unknown.w2d");
    let compact = compact.to_str().expect("a utf-8 path");

    dataset.save_to_sqlite(&sqlite).expect("it should save");
    dataset
        .save(compact, Format::Compact)
        .expect("it should save");

    let id = ExpressionId::new(WorkId::new("westlaw/headnote_1"), "2015-06-26");
    for (form, stored) in [
        (
            "sqlite",
            Dataset::open_sqlite(&sqlite)
                .expect("it should reopen")
                .get_expression(&id)
                .expect("it should answer")
                .expect("the node should be held"),
        ),
        (
            "compact",
            Dataset::load(compact, Format::Compact)
                .expect("it should reopen")
                .get_expression(&id)
                .expect("it should answer")
                .expect("the node should be held"),
        ),
    ] {
        assert_eq!(
            stored.root.data.node_type.as_str(),
            "westlaw.headnote",
            "{form} should not drop a node type it does not know"
        );
        let payload = stored
            .root
            .data
            .payload_in("westlaw")
            .unwrap_or_else(|| panic!("{form} should hand back the payload"));
        assert_eq!(
            &*payload.value, unknown_facts,
            "{form} should hand the payload back byte for byte"
        );
    }
}
