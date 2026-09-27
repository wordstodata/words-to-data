//! The Office of Law Revision Counsel's classification tables (#247).
//!
//! Every test reads the real page the OLRC published for the 119th Congress,
//! 1st session, in public law order, committed under
//! `tests/test_data/olrc/classification`. No row here is made up.

use std::sync::OnceLock;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Scope, WorkId};
use words_to_data::link::{LinkKind, Target, VerificationState};
use words_to_data::olrc::{ClassificationRow, ClassificationTable, SkipReason, classify};

/// The committed page, as the OLRC served it.
const PUBLIC_LAW_ORDER: &str = "tests/test_data/olrc/classification/tbl119pl_1st.htm";

fn committed_table() -> ClassificationTable {
    let html = std::fs::read_to_string(PUBLIC_LAW_ORDER).expect("the page is committed");
    ClassificationTable::parse(&html).expect("the committed page should parse")
}

#[test]
fn should_read_every_row_when_the_page_is_a_real_table() {
    let table = committed_table();

    assert_eq!(table.rows.len(), 3049, "the page lists 3,049 classifications");

    let first = &table.rows[0];
    assert_eq!(first.title, "8");
    assert_eq!(first.section, "1101");
    assert_eq!(first.description, "nt new");
    assert_eq!(first.public_law, "119-1");
    assert_eq!(first.law_sections, "1");
    assert_eq!(first.statutes_page, "3");
}

/// The row of Public Law 119-21 whose sections column reads `written`.
fn row_of_119_21(table: &ClassificationTable, written: &str) -> ClassificationRow {
    table
        .rows
        .iter()
        .find(|row| row.public_law == "119-21" && row.law_sections == written)
        .unwrap_or_else(|| panic!("the table lists 119-21 {written}"))
        .clone()
}

#[test]
fn should_name_each_law_section_when_a_row_lists_two() {
    let table = committed_table();
    let row = row_of_119_21(&table, "71301(a), (b)");

    assert_eq!(row.section, "36B");
    assert_eq!(row.named_law_sections(), vec!["71301(a)", "71301(b)"]);
}

#[test]
fn should_keep_the_designations_above_a_continuation_when_it_names_a_deeper_level() {
    let table = committed_table();

    let paragraph = row_of_119_21(&table, "70431(a)(4)(B), (5)");
    assert_eq!(
        paragraph.named_law_sections(),
        vec!["70431(a)(4)(B)", "70431(a)(5)"]
    );

    let clause = row_of_119_21(&table, "70323(a)(3)(A)(i), (ii)");
    assert_eq!(
        clause.named_law_sections(),
        vec!["70323(a)(3)(A)(i)", "70323(a)(3)(A)(ii)"]
    );

    let subsection = row_of_119_21(&table, "71305(a), (b)(1)");
    assert_eq!(
        subsection.named_law_sections(),
        vec!["71305(a)", "71305(b)(1)"]
    );
}

#[test]
fn should_leave_out_the_quoted_code_section_when_a_row_names_a_new_one() {
    let table = committed_table();
    let row = row_of_119_21(&table, "70302(a) \"174A\"");

    // The quotation names the section added to the Code, which the row's own
    // section column already states. It is not a section of the law.
    assert_eq!(row.section, "174A");
    assert_eq!(row.named_law_sections(), vec!["70302(a)"]);
}

/// The table page the committed fixture is, as a link's source names it.
const TABLE_SOURCE: &str = "olrc:https://usc-cdn.house.gov/classification/tbl119pl_1st.htm";

/// Title 26 as the 2025-07-30 release point publishes it: what the dataset
/// holds, and where its sections are. Read once, because the title is 55 MB.
fn title_26() -> &'static (Scope, SectionPaths) {
    static HELD: OnceLock<(Scope, SectionPaths)> = OnceLock::new();
    HELD.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata {
            name: "OLRC classification fixture".to_string(),
            ..Default::default()
        });
        dataset
            .add_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30", None)
            .expect("title 26 should parse");
        let expression = dataset
            .get_expression(&ExpressionId::new(
                WorkId::new("uscode/title_26"),
                "2025-07-30",
            ))
            .expect("storage should answer")
            .expect("title 26 should be held");
        let mut paths = SectionPaths::new();
        paths.add_work(&expression.root);
        (dataset.scope().expect("the scope should derive"), paths)
    })
}

#[test]
fn should_store_one_asserted_link_per_law_section_when_the_dataset_holds_the_code_section() {
    let table = committed_table();
    let row = row_of_119_21(&table, "71301(a), (b)");
    let (scope, paths) = title_26();

    let classified = classify(&[row], scope, paths, TABLE_SOURCE);

    assert!(classified.skipped.is_empty(), "section 36B is held");
    assert_eq!(classified.links.len(), 2, "one link per section of the law");

    let objects: Vec<String> = classified
        .links
        .iter()
        .map(|link| link.object.name())
        .collect();
    assert_eq!(
        objects,
        vec![
            "olrc.classification:119-21:71301(a)",
            "olrc.classification:119-21:71301(b)"
        ]
    );

    for link in &classified.links {
        let Target::Node(path) = &link.subject else {
            panic!("the subject is the Code section, by its path");
        };
        assert!(path.starts_with("uscode/title_26/"), "{path}");
        assert!(path.ends_with("/section_36B"), "{path}");
        assert_eq!(link.kind, LinkKind::new(LinkKind::CLASSIFIED_FROM));
        assert_eq!(link.provenance.verification, VerificationState::Asserted);
        assert_eq!(link.provenance.source, TABLE_SOURCE);

        let payload = link.payload.as_ref().expect("the kind of change");
        assert_eq!(payload.namespace, "olrc");
        assert_eq!(payload.value["description"], "", "36B is amended");
    }
}

#[test]
fn should_find_the_code_section_when_the_table_writes_its_dash_as_a_hyphen() {
    let table = committed_table();
    let row = row_of_119_21(&table, "70421(a)(5)");
    let (scope, paths) = title_26();

    let classified = classify(std::slice::from_ref(&row), scope, paths, TABLE_SOURCE);

    // The table writes `1400Z-1`; the Code, and so the path, writes an en dash.
    assert_eq!(row.section, "1400Z-1");
    assert!(classified.skipped.is_empty(), "section 1400Z–1 is held");
    let Target::Node(path) = &classified.links[0].subject else {
        panic!("the subject is the Code section, by its path");
    };
    assert!(path.ends_with("/section_1400Z\u{2013}1"), "{path}");
}

#[test]
fn should_skip_a_row_as_out_of_scope_when_the_dataset_does_not_hold_its_title() {
    let table = committed_table();
    let row = row_of_119_21(&table, "71103(a)(1)");
    let (scope, paths) = title_26();

    let classified = classify(std::slice::from_ref(&row), scope, paths, TABLE_SOURCE);

    assert_eq!(row.title, "42", "the row names title 42, which is not held");
    assert!(classified.links.is_empty());
    assert_eq!(classified.skipped.len(), 1);
    assert_eq!(classified.skipped[0].reason, SkipReason::TitleNotHeld);
}
