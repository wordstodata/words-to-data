//! The Office of Law Revision Counsel's classification tables (#247).
//!
//! Every test reads the real page the OLRC published for the 119th Congress,
//! 1st session, in public law order, committed under
//! `tests/test_data/olrc/classification`. No row here is made up.

use words_to_data::olrc::{ClassificationRow, ClassificationTable};

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
