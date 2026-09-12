use std::fs::File;
use std::io::BufReader;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId, work_roots,
};
use words_to_data::diff::TreeDiff;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_parser::parse_bill_amendments;
use words_to_data::uslm::parser::parse;

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// Title 7 is Agriculture. It is the work every expression below belongs to.
const TITLE_7: &str = "uscode/title_7";

fn title_7() -> WorkId {
    WorkId::new(TITLE_7)
}

fn at(date: &str) -> ExpressionId {
    ExpressionId::new(title_7(), date)
}

#[test]
fn should_serialize_roundtrip_json() {
    let metadata = DatasetMetadata {
        name: "Test Dataset".to_string(),
        description: "For testing".to_string(),
        author: "Test".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "0.1.0".to_string(),
        ..Default::default()
    };

    let mut dataset = Dataset::new(metadata);

    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-18",
            Some("test".to_string()),
        )
        .expect("failed to load USLM doc");
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-30",
            Some("test".to_string()),
        )
        .expect("failed to load USLM doc");

    let annotations = make_annotations();
    for annotation in annotations.into_iter() {
        store_annotation(
            &mut dataset,
            annotation,
            &at("2025-07-18"),
            &at("2025-07-30"),
        );
    }

    // Save and load via Compact format (JSON with tuple keys requires file-based roundtrip)
    let path = "/tmp/dataset_test_roundtrip.json";
    dataset.save(path, Format::Compact).unwrap();
    let roundtripped = Dataset::load(path, Format::Compact).unwrap();

    assert_eq!(roundtripped.metadata().name, "Test Dataset");
    let expressions = roundtripped.expressions(&title_7()).unwrap();
    assert_eq!(expressions.len(), 2);
    assert_eq!(expressions[0].id, at("2025-07-18"));
    assert_eq!(expressions[0].label, Some("test".to_string()));
    // Links are what is stored now, and they regroup: one record carries every
    // path one amendment touched, per asserter. So the record count is no
    // longer the fixture's 753. What must survive is the *facts* — the distinct
    // (amendment, path) statements. The fixture's 753 single-path records hold
    // 718 of them; the other 35 restate one already there, and a link is
    // identified by what it says, so a restatement updates rather than adds
    // (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    let annotations = roundtripped
        .get_annotations(&at("2025-07-18"), &at("2025-07-30"))
        .unwrap()
        .unwrap();
    let facts: std::collections::HashSet<(String, String)> = annotations
        .iter()
        .flat_map(|a| {
            a.paths
                .iter()
                .map(|p| (a.source_bill.amendment_id.clone(), p.clone()))
        })
        .collect();
    assert_eq!(
        facts.len(),
        718,
        "every distinct (amendment, path) statement must survive the round trip"
    );
    assert_eq!(
        annotations.len(),
        317,
        "one record per amendment per asserter, not one per path"
    );
    assert!(
        roundtripped
            .get_annotations(&at("2025-07-18"), &at("2025-07-20"))
            .unwrap()
            .is_none()
    );

    std::fs::remove_file(path).ok();
}

fn make_test_dataset() -> Dataset<InMemoryStorage> {
    let metadata = DatasetMetadata {
        name: "Test".to_string(),
        description: "Test".to_string(),
        author: "Test".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "0.1.0".to_string(),
        ..Default::default()
    };
    Dataset::new(metadata)
}

/// Title 7's tree, labelled with `date`. The file is one real release of title
/// 7; the date is a label on it, which is what lets these tests pin ordering
/// without needing three real releases on disk.
fn make_expression(date: &str, label: Option<&str>) -> Expression {
    let parsed = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    let root = work_roots(parsed).pop().expect("the file holds one title");
    Expression {
        id: at(date),
        label: label.map(|s| s.to_string()),
        element: root,
    }
}

#[test]
fn should_hold_expressions_of_one_work_in_date_order() {
    let mut dataset = make_test_dataset();

    // Add out of order
    dataset
        .add_expression(make_expression("2024-06-01", None))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-01-01", Some("First")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-12-01", Some("Last")))
        .unwrap();

    let ids: Vec<ExpressionId> = dataset
        .expressions(&title_7())
        .unwrap()
        .into_iter()
        .map(|info| info.id)
        .collect();
    assert_eq!(
        ids,
        vec![at("2024-01-01"), at("2024-06-01"), at("2024-12-01")]
    );
}

/// Adding the same work and date twice replaces rather than duplicating. Two
/// trees both claiming to be title 7 on one day is not a state a reader can
/// resolve.
#[test]
fn should_replace_an_expression_when_the_same_work_and_date_is_added_twice() {
    let mut dataset = make_test_dataset();

    dataset
        .add_expression(make_expression("2024-01-01", Some("First")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-01-01", Some("Corrected")))
        .unwrap();

    let expressions = dataset.expressions(&title_7()).unwrap();
    assert_eq!(expressions.len(), 1);
    assert_eq!(expressions[0].label, Some("Corrected".to_string()));
}

#[test]
fn should_get_an_expression_by_its_id() {
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2024-01-01", Some("First")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-06-01", None))
        .unwrap();

    let found = dataset.get_expression(&at("2024-01-01")).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().label, Some("First".to_string()));

    let not_found = dataset.get_expression(&at("2024-03-01")).unwrap();
    assert!(not_found.is_none());
}

#[test]
fn should_navigate_expressions_of_one_work() {
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2024-01-01", Some("First")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-06-01", Some("Middle")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-12-01", Some("Last")))
        .unwrap();

    let next = dataset.next_expression(&at("2024-01-01")).unwrap();
    assert_eq!(next.map(|e| e.id), Some(at("2024-06-01")));

    let prev = dataset.prev_expression(&at("2024-12-01")).unwrap();
    assert_eq!(prev.map(|e| e.id), Some(at("2024-06-01")));

    // Edge cases
    assert!(
        dataset
            .next_expression(&at("2024-12-01"))
            .unwrap()
            .is_none()
    ); // no next after last
    assert!(
        dataset
            .prev_expression(&at("2024-01-01"))
            .unwrap()
            .is_none()
    ); // no prev before first
}

/// The label names one printing of one work. It survives a save and load, and
/// it is reported beside the id rather than being a key of its own: two works
/// may reasonably carry the same label.
#[test]
fn should_carry_a_label_alongside_each_expression() {
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2024-01-01", Some("Pre-Tax Cuts Act")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-06-01", None))
        .unwrap();

    let expressions = dataset.expressions(&title_7()).unwrap();
    assert_eq!(expressions[0].label, Some("Pre-Tax Cuts Act".to_string()));
    assert_eq!(expressions[1].label, None);
}

#[test]
fn should_save_and_load_file() {
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2024-01-01", Some("First")))
        .unwrap();

    let path = "/tmp/dataset_test_save_load.json";

    // Save
    dataset
        .save(path, Format::Compact)
        .expect("save should succeed");

    // Load
    let loaded = Dataset::load(path, Format::Compact).expect("load should succeed");

    assert_eq!(loaded.metadata().name, "Test");
    assert_eq!(loaded.works().unwrap(), vec![title_7()]);
    let expressions = loaded.expressions(&title_7()).unwrap();
    assert_eq!(expressions.len(), 1);
    assert_eq!(expressions[0].id, at("2024-01-01"));

    // Cleanup
    std::fs::remove_file(path).ok();
}

#[test]
fn should_compute_diff_between_two_expressions_of_one_work() {
    let mut dataset = make_test_dataset();

    // Use two real releases of title 7
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-18",
            Some("First".to_string()),
        )
        .unwrap();
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc07.xml",
            "2025-07-30",
            Some("Second".to_string()),
        )
        .unwrap();

    let diff: TreeDiff = dataset
        .compute_diff(&at("2025-07-18"), &at("2025-07-30"))
        .unwrap();

    // The diff is rooted at the work, not at the container the release
    // point happened to arrive in.
    assert_eq!(diff.root_path, TITLE_7);
}

#[test]
fn should_add_and_query_bills() {
    let mut dataset = make_test_dataset();

    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    let bill_id = bill.bill_id.clone();

    dataset.add_bill(bill).unwrap();

    assert_eq!(dataset.storage().bills.len(), 1);

    let found = dataset.get_bill(&bill_id).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().bill_id, "119-21");

    let not_found = dataset.get_bill("nonexistent").unwrap();
    assert!(not_found.is_none());
}

#[test]
fn should_list_all_bill_ids_when_iterating_dataset() {
    let mut dataset = make_test_dataset();

    assert!(dataset.list_bill_ids().unwrap().is_empty());

    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    dataset.add_bill(bill).unwrap();

    let ids = dataset.list_bill_ids().unwrap();
    assert_eq!(ids, vec!["119-21".to_string()]);
}

fn make_annotations() -> Vec<ChangeAnnotation> {
    let file = File::open("tests/test_data/processed/annotations.json")
        .expect("should be able to open annotations file");
    let annotations: Vec<ChangeAnnotation> = serde_json::from_reader(BufReader::new(file)).unwrap();
    annotations
}

#[test]
fn should_query_annotations_by_path() {
    let mut dataset = make_test_dataset();

    for annotation in make_annotations().into_iter() {
        store_annotation(
            &mut dataset,
            annotation,
            &at("2025-07-18"),
            &at("2025-07-30"),
        );
    }

    // Query by path
    let found = dataset.annotations_for_path("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_163/subsection_j/paragraph_8/subparagraph_A/clause_v").unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].source_bill.bill_id, "119-21");

    // Query by bill. Answered from the link's object reference, which now
    // names the bill as well as the amendment, so the `bill_id` column that
    // predates the core/extension split is gone.
    let found = dataset.annotations_for_bill("119-21").unwrap();
    assert_eq!(
        found.len(),
        317,
        "records regroup by amendment and asserter; the whole fixture is one bill"
    );
    let paths: usize = found.iter().map(|a| a.paths.len()).sum();
    assert_eq!(paths, 718, "every distinct statement is still reachable");

    // No matches
    let found = dataset
        .annotations_for_path("uscode/title_99/section_1")
        .unwrap();
    assert_eq!(found.len(), 0);
}

#[test]
fn should_find_element_across_expressions() {
    let mut dataset = make_test_dataset();

    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-18",
            None,
        )
        .unwrap();
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc07.xml",
            "2025-07-30",
            None,
        )
        .unwrap();

    let results = dataset.find_element(TITLE_7).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, at("2025-07-18"));
    assert_eq!(results[1].0, at("2025-07-30"));
}

#[test]
fn should_search_text_across_expressions() {
    let mut dataset = make_test_dataset();

    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-18",
            None,
        )
        .unwrap();

    // Search for text that exists in Title 7 (Agriculture)
    let results = dataset.search_text("Agriculture").unwrap();

    // Should find at least one match
    assert!(!results.is_empty());
    // A hit names the expression it was found in, not a bare date: the date
    // alone cannot say which document the text belongs to.
    assert_eq!(results[0].expression, at("2025-07-18"));
}

/// Store an annotation the way the pipeline does: one link per path it names.
///
/// There is deliberately no writer convenience for this in the library — two
/// ways to write one fact means the convenient one is used, and it could only
/// express the single kind we own (`docs/adr/0004`).
fn store_annotation<S: words_to_data::storage::Storage>(
    dataset: &mut Dataset<S>,
    annotation: ChangeAnnotation,
    from: &ExpressionId,
    to: &ExpressionId,
) {
    for link in words_to_data::link::Link::from_annotation(&annotation, from, to) {
        dataset.add_link(link).expect("the link should be added");
    }
}
