//! The Office of Law Revision Counsel's classification tables (#247).
//!
//! Every test reads the real page the OLRC published for the 119th Congress,
//! 1st session, in public law order, committed under
//! `tests/test_data/olrc/classification`. No row here is made up.

use words_to_data::olrc::ClassificationTable;

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
