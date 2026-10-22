//! A court opinion stores honestly, and nothing about it says US Code.
//!
//! Before #129 an opinion could only be stored by declaring a `DocumentType` of
//! US Code or bill, and by taking an `ElementType` out of USLM's legislative
//! vocabulary. Both are false statements about what the document is, written into
//! a file and sent to another party. This is the test that they are gone.
//!
//! The records are real and cached, not written by hand: *Obergefell v. Hodges*,
//! CourtListener opinion 2812209, with its cluster beside it. **The filing date is
//! on the cluster.** The opinion's own `date_created` is when CourtListener
//! ingested the record, and using it would date the expression to the day a third
//! party scraped it.

use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId};
use words_to_data::document::{ClassPayload, DocumentNode, NodeData, NodeType, text_method};
use words_to_data::inspect;
use words_to_data::judicial::{CaseFacts, OpinionFacts};
use words_to_data::link::VerificationState;
use words_to_data::method::Method;
use words_to_data::uslm::UslmFacts;

const OPINION_JSON: &str = "tests/test_data/courtlistener/opinion_2812209.json";
const CLUSTER_JSON: &str = "tests/test_data/courtlistener/cluster_2812209.json";

/// The work a case is an expression of: `judicial/case_2812209`, by the
/// publisher's cluster id.
///
/// A structural path on the same rule as `uscode/title_26`: a class segment, then
/// `<kind>_<number>`. The number is the publisher's id, which is the only stable
/// name the source gives.
const WORK: &str = "judicial/case_2812209";

/// The Court's opinion. CourtListener holds *Obergefell* as one combined
/// record, 2812209, with every opinion of the case in it, and a rule splits it
/// into its five writings; this is the first (#315).
const WRITING: &str = "judicial/case_2812209/opinion_2812209-1";

/// A sentence from the middle of the opinion, used to prove that search reaches
/// the text rather than the heading.
///
/// Not the words before it. Harvard's text, which is the text read since ADR
/// 0014 put structure first, prints "promises libertyto all": OCR, as its
/// provenance says.
const FROM_THE_OPINION: &str = "a liberty that includes certain specific rights";

/// Build the expression this dataset would store for the cached case, from the
/// two committed records.
fn obergefell() -> Expression {
    let read = |path| std::fs::read_to_string(path).expect("the cached record should read");
    let opinion = OpinionRecord::from_json(&read(OPINION_JSON)).expect("the opinion should read");
    let cluster = ClusterRecord::from_json(&read(CLUSTER_JSON)).expect("the cluster should read");
    case_expression(&cluster, &[opinion])
        .expect("the case should build")
        .expression
}

/// The Court's opinion.
fn writing(expression: &Expression) -> &DocumentNode {
    expression
        .root
        .find(WRITING)
        .expect("the case holds its writing")
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
fn should_store_the_opinion_as_one_writing_of_its_case_dated_from_the_cluster() {
    let dataset = dataset_holding_the_opinion();

    let id = ExpressionId::new(WorkId::new(WORK), "2015-06-26");
    let stored = dataset
        .get_expression(&id)
        .expect("the dataset should answer")
        .expect("the case should be held");

    assert_eq!(stored.root.data.node_type.as_str(), "judicial.case");
    let opinion = writing(&stored);
    assert_eq!(opinion.data.node_type.as_str(), "judicial.opinion");
    assert_eq!(opinion.data.date.to_string(), "2015-06-26");
    // Since #314 a writing read from Harvard's field holds its words in its
    // paragraphs and footnotes, and none itself, so search never returns the
    // same words twice.
    assert_eq!(opinion.data.content, None);
    let mut under = Vec::new();
    for child in &opinion.children {
        nodes(child, &mut under);
    }
    let holding: Vec<&str> = under
        .iter()
        .filter(|data| {
            data.content
                .as_deref()
                .is_some_and(|text| text.contains(FROM_THE_OPINION))
        })
        .map(|data| &*data.path)
        .collect();
    assert_eq!(
        holding,
        [format!("{WRITING}/paragraph_2")],
        "the Court's paragraph alone: the dissent's footnote that quotes it is in \
         the dissent's own writing since #315"
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

/// The same statement from the other side: the nodes carry no USLM facts at all.
#[test]
fn should_carry_no_uslm_facts_when_the_node_is_an_opinion() {
    let expression = obergefell();

    for data in [&expression.root.data, &writing(&expression).data] {
        assert!(
            UslmFacts::of(data).is_none(),
            "a USLM reader has nothing to say about a court opinion"
        );
        assert_eq!(data.node_type.namespace(), NodeType::JUDICIAL);
    }
}

/// `extracted_by_ocr` says how the text was obtained, so it is provenance rather
/// than a judicial fact. A reader gets it without opening any payload.
#[test]
fn should_reach_how_the_text_was_obtained_without_opening_a_payload() {
    let expression = obergefell();
    let opinion = writing(&expression);

    let provenance = opinion
        .data
        .provenance
        .as_ref()
        .expect("an opinion records where its text came from");

    assert_eq!(
        provenance.source,
        "courtlistener:opinion/2812209:xml_harvard"
    );
    assert_eq!(
        provenance.method.as_ref(),
        Some(&Method::new(text_method::OCR, text_method::VERSION)),
        "the text is Harvard's, which CAP says is machine OCR, although this \
         record says extracted_by_ocr is false; the version says which reading \
         produced it"
    );
    assert_eq!(provenance.verification, VerificationState::MachineSuggested);

    // And the payload says nothing about it, so there is one place to look.
    let payload = opinion
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

    let case = CaseFacts::of(&stored.root.data).expect("the case facts should come back");
    assert_eq!(case.case_name, "Obergefell v. Hodges");
    assert_eq!(case.cluster_id, 2812209);
    assert_eq!(case.precedential_status.as_deref(), Some("Published"));
    assert_eq!(case.source.as_deref(), Some("CU"));
    assert!(
        case.citations.contains(&"576 U.S. 644".to_string()),
        "the reporters' citations should survive, got {:?}",
        case.citations
    );

    let opinion = writing(&stored);
    let facts = OpinionFacts::of(&opinion.data).expect("the writing facts should come back");
    assert_eq!(
        facts.opinion_type.as_deref(),
        Some("020lead"),
        "the rule read the writing's kind from the line that opens it"
    );
    assert_eq!(facts.author.as_deref(), Some("Kennedy"));
    assert_eq!(facts.page_count, Some(103));

    // Provenance is core, so it travels beside the payload rather than in it.
    assert_eq!(
        opinion
            .data
            .provenance
            .as_ref()
            .and_then(|p| p.method.clone()),
        Some(Method::new(text_method::OCR, text_method::VERSION))
    );
    assert_eq!(opinion.data.node_type.as_str(), "judicial.opinion");
}

#[test]
fn should_report_the_opinion_through_search_expressions_and_info() {
    let dataset = dataset_holding_the_opinion();

    let hits = inspect::search(&dataset, FROM_THE_OPINION).expect("the search should run");
    let paths: Vec<&str> = hits.iter().map(|hit| &*hit.path).collect();
    assert_eq!(
        paths,
        [
            &format!("{WRITING}/paragraph_2"),
            "judicial/case_2812209/opinion_2812209-3/footnote_22"
        ],
        "search should reach the opinion's text, in the passage that holds it, \
         and Justice Scalia's footnote that quotes it, in his own writing"
    );

    let listed = inspect::expressions(&dataset, None).expect("the listing should run");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "judicial/case_2812209@2015-06-26");
    let mut built = Vec::new();
    nodes(&obergefell().root, &mut built);
    assert!(built.len() > 3, "the writing holds its passages");
    assert_eq!(
        listed[0].element_count,
        built.len(),
        "the case, its head matter, its one writing and each passage of it, \
         each counted"
    );

    let info = inspect::info(&dataset).expect("info should run");
    assert_eq!(info.work_count, 1);
    assert_eq!(info.expression_count, 1);
    assert!(
        info.legislature.is_none(),
        "a judicial dataset does not speak legislature, so it reports no counts"
    );
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
