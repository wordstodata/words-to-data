//! A list of instructions under a line that names a whole title (#365).
//!
//! Pub. L. 119-60 § 1701(a) says *"Title 10, United States Code, is amended as
//! follows:"*, and each of its 43 paragraphs names the section it acts on:
//!
//! > (10) In chapter 40, by redesignating section 711 (relating to parental
//! > leave …) as section 710a …
//!
//! The line names no section, so read as one instruction the list has no
//! address. Each paragraph is its own amendment, and the section it names is
//! its address.
//!
//! Every case reads the committed public law `119-s-1071`. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::document::DocumentNode;
use words_to_data::legislature::redesignation::{Reason, StatedRedesignation};
use words_to_data::link::{LinkKind, Target};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::amendment_address::{AmendmentAddress, addresses_in};
use words_to_data::uslm::bill_parser::bill_expression;
use words_to_data::uslm::bill_redesignation::{redesignations_stated, redesignations_stated_in};

const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";

/// Public Law 119-60, as the dataset stores it. Read once: the file is 12 MB.
fn committed_ndaa() -> &'static DocumentNode {
    static BILL: OnceLock<DocumentNode> = OnceLock::new();
    BILL.get_or_init(|| {
        let xml = std::fs::read_to_string(NDAA).expect("the public law should be committed");
        let document = roxmltree::Document::parse(&xml).expect("the public law should parse");
        let (bill, _) = bill_expression(&document, NDAA_ID).expect("the bill should read");
        bill.root
    })
}

/// The one address whose instruction's own words hold `phrase`.
fn address_saying(addresses: &[AmendmentAddress], phrase: &str) -> AmendmentAddress {
    let found: Vec<&AmendmentAddress> = addresses
        .iter()
        .filter(|address| address.text.contains(phrase))
        .collect();
    let [one] = found[..] else {
        panic!(
            "exactly one instruction should say {phrase:?}, found {}",
            found.len()
        );
    };
    one.clone()
}

/// § 1701(a)(10) is an amendment of its own, at its own place in the bill,
/// and it acts on 10 U.S.C. 711.
#[test]
fn should_address_the_section_a_paragraph_names_when_its_line_amends_a_whole_title() {
    let addresses = addresses_in(committed_ndaa());

    let address = address_saying(&addresses, "by redesignating section 711 (relating to");

    assert!(
        address
            .path
            .ends_with("section_1701/subsection_a/paragraph_10"),
        "the paragraph should be its own amendment: {}",
        address.path
    );
    assert_eq!(address.section.as_deref(), Some("/us/usc/t10/s711"));
}

/// Title 10 as it read before and after Public Law 119-60.
const BEFORE: &str = "2025-08-14";
const AFTER: &str = "2026-01-23";

/// Title 10 at both release points, with the bill's renumberings recorded as
/// links over the window between them.
fn title_10_with_the_renumberings() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for date in [BEFORE, AFTER] {
        common::add_uslm_xml(
            &mut dataset,
            &format!("tests/test_data/usc/{date}/usc10.xml"),
            date,
        );
    }
    let work = WorkId::new("uscode/title_10");
    let stated = redesignations_stated_in(NDAA_ID, committed_ndaa());
    dataset
        .record_redesignations(
            NDAA_ID,
            &stated,
            &ExpressionId::new(work.clone(), BEFORE),
            &ExpressionId::new(work, AFTER),
        )
        .expect("the renumberings should record");
    dataset
}

/// § 1701(a)(10) renumbers 10 U.S.C. 711 in chapter 40 as § 710a. Title 10
/// holds a second § 711, in chapter 41, and the item's *"In chapter 40"* says
/// which one moved.
#[test]
fn should_record_the_renumbered_section_when_an_item_of_the_list_renumbers_a_section() {
    let dataset = title_10_with_the_renumberings();
    let chapter_40 = "uscode/title_10/subtitle_A/part_II/chapter_40";

    let links = dataset
        .links_for_path(&format!("{chapter_40}/section_711"))
        .expect("links should read");
    let renumbered: Vec<&Target> = links
        .iter()
        .filter(|link| link.kind.0 == LinkKind::REDESIGNATED_AS)
        .map(|link| &link.object)
        .collect();

    assert_eq!(
        renumbered,
        [&Target::Change {
            work: WorkId::new("uscode/title_10"),
            path: format!("{chapter_40}/section_710a"),
            from_date: BEFORE.to_string(),
            to_date: AFTER.to_string(),
        }]
    );
}

/// Pub. L. 119-60 § 3111(a):
///
/// > Part VI of subtitle A of title 10, United States Code, is [10 USC prec.
/// > 101.] amended by adding at the end the following new subpart: …
///
/// The publisher's marginal note sits between *is* and *amended*, and the
/// line is still the amending line. It names a part and no section, and the
/// new subpart holds many sections, so the reason is that no section was
/// named, and not that nothing says what the clause amends.
#[test]
fn should_read_the_amending_line_when_a_marginal_note_sits_inside_is_amended() {
    let addresses = addresses_in(committed_ndaa());

    let address = address_saying(&addresses, "Part VI of subtitle A of title 10");

    assert_eq!(
        address.unresolved,
        Some(Reason::NoSectionNamed),
        "{address:?}"
    );
}

/// Pub. L. 119-60 § 7701(f):
///
/// > Title 46, United States Code, is amended as follows:
/// > (1) Section 2101(2) is amended by striking "section 1" and inserting
/// > "section 101".
///
/// The item's own line cites a bare section, and the list says which title.
#[test]
fn should_take_the_title_from_the_list_when_an_item_cites_a_bare_section() {
    let addresses = addresses_in(committed_ndaa());

    let address = address_saying(&addresses, "Section 2101(2) is amended by striking");

    assert_eq!(
        address.section.as_deref(),
        Some("/us/usc/t46/s2101"),
        "{address:?}"
    );
    assert_eq!(
        address
            .container
            .iter()
            .map(|step| step.number.as_str())
            .collect::<Vec<_>>(),
        ["2"]
    );
}

/// Both readers of a bill place the renumbering in § 1701(a)(10) at 10 U.S.C.
/// 711, and name the same amendment: the item, not the whole list.
#[test]
fn should_give_both_readers_the_section_and_the_amendment_when_an_item_of_the_list_renumbers() {
    let xml = std::fs::read_to_string(NDAA).expect("the public law should be committed");
    let from_markup = redesignations_stated(NDAA_ID, &xml).expect("the markup should read");
    let from_document = redesignations_stated_in(NDAA_ID, committed_ndaa());
    let item = address_saying(
        &addresses_in(committed_ndaa()),
        "by redesignating section 711 (relating to",
    );

    for (reader, stated) in [("markup", from_markup), ("document", from_document)] {
        let found: Vec<&StatedRedesignation> = stated
            .iter()
            .filter(|statement| statement.text.contains("redesignating section 711"))
            .collect();
        let [statement] = found[..] else {
            panic!("the {reader} reader should state the renumbering once: {found:?}");
        };
        assert_eq!(
            statement.section.as_deref(),
            Some("/us/usc/t10/s711"),
            "the {reader} reader"
        );
        assert_eq!(
            statement.amendment_id, item.amendment_id,
            "the {reader} reader"
        );
    }
}

/// Pub. L. 119-60 § 1701(a)(1):
///
/// > In the chapter analysis for subtitle A, by striking the item relating to
/// > chapter 243 and inserting the following: …
///
/// The item names an index and no section, so it is an amendment of its own
/// with no address, and the reason is the one the whole list had before.
#[test]
fn should_keep_the_reason_when_an_item_of_the_list_names_no_section() {
    let addresses = addresses_in(committed_ndaa());

    let address = address_saying(&addresses, "In the chapter analysis for subtitle A");

    assert!(
        address
            .path
            .ends_with("section_1701/subsection_a/paragraph_1")
    );
    assert_eq!(address.unresolved, Some(Reason::NoSectionNamed));
}

/// Pub. L. 119-60 § 1701(a)(3):
///
/// > (3) In section 132a—
/// > (A) in subsection (a)(1), …
///
/// The dash after the number ends it. § 132a- is no section.
#[test]
fn should_end_the_section_number_at_the_dash_when_an_item_opens_a_scope() {
    let addresses = addresses_in(committed_ndaa());

    let address = addresses
        .iter()
        .find(|address| {
            address
                .path
                .ends_with("section_1701/subsection_a/paragraph_3")
        })
        .expect("the item should be an amendment");

    assert_eq!(address.section.as_deref(), Some("/us/usc/t10/s132a"));
}

/// Pub. L. 119-60 § 811(c)(2):
///
/// > (2) Amending provisions that refer to section 3455(f) so as to refer to
/// > section 101(a) instead.—The following sections of title 10, United States
/// > Code, are each amended by striking "section 3455(f)" and inserting
/// > "section 101(a)":
/// > (A) Section 3456(a). …
///
/// The heading quotes the words the list changes. It does not say where the
/// list acts, so no item is addressed to § 3455.
#[test]
fn should_not_address_the_section_a_heading_names_when_the_items_name_their_own() {
    let addresses = addresses_in(committed_ndaa());

    let sections: Vec<Option<&str>> = addresses
        .iter()
        .filter(|address| {
            address
                .path
                .contains("section_811/subsection_c/paragraph_2/")
        })
        .map(|address| address.section.as_deref())
        .collect();

    assert!(!sections.is_empty(), "each item should be an amendment");
    assert!(
        !sections.contains(&Some("/us/usc/t10/s3455")),
        "{sections:?}"
    );
}

/// Pub. L. 119-60 § 7301(d)(1) (#438):
///
/// > (d) Chapter 73 of title 46, United States Code, is amended—
/// > (1) by amending section 7306 to read as follows: …
///
/// The line names a chapter, and the item names the section it acts on after
/// *by amending*. The list gives the title.
#[test]
fn should_address_the_section_an_item_amends_when_the_item_says_by_amending_section() {
    let addresses = addresses_in(committed_ndaa());

    let address = address_saying(&addresses, "by amending section 7306 to read as follows");

    assert!(
        address
            .path
            .ends_with("section_7301/subsection_d/paragraph_1"),
        "the item should be its own amendment: {}",
        address.path
    );
    assert_eq!(
        address.section.as_deref(),
        Some("/us/usc/t46/s7306"),
        "{address:?}"
    );
}

/// Pub. L. 119-60 § 7301(d)(3) (#438):
///
/// > (3) [46 USC 7308.] in section 7308 by striking "18 months" …
///
/// The publisher's marginal note sits before *in section*, and the item still
/// names the section it acts on.
#[test]
fn should_address_the_section_an_item_names_when_a_marginal_note_opens_the_item() {
    let addresses = addresses_in(committed_ndaa());

    let address = addresses
        .iter()
        .find(|address| {
            address
                .path
                .ends_with("section_7301/subsection_d/paragraph_3")
        })
        .expect("the item should be an amendment");

    assert_eq!(
        address.section.as_deref(),
        Some("/us/usc/t46/s7308"),
        "{address:?}"
    );
}

/// Pub. L. 119-60 § 7201(e)(5) (#438):
///
/// > (e) Chapter 3 of title 14, United States Code, is amended—
/// > (5) by inserting after section 309 (as so redesignated …) the following:
/// > "§ 310. Director of Coast Guard Investigative Service …
///
/// § 309 is only where the new text goes. The item inserts a whole new
/// section, and the heading of the text it enacts gives its number.
#[test]
fn should_address_the_new_section_when_an_item_of_the_list_inserts_one() {
    let addresses = addresses_in(committed_ndaa());

    let address = addresses
        .iter()
        .find(|address| {
            address
                .path
                .ends_with("section_7201/subsection_e/paragraph_5")
        })
        .expect("the item should be an amendment");

    assert_eq!(
        address.section.as_deref(),
        Some("/us/usc/t14/s310"),
        "{address:?}"
    );
}
