//! One amending instruction that acts on many sections (#434, gap 2).
//!
//! Some instructions name no section in their own line. They name an Act, and
//! each item of the list below the line names the section it acts on:
//!
//! > (A) The National Security Act of 1947 (50 U.S.C. 3001 et seq.) is
//! > amended—
//! > (i) in section 103(c)(15) (50 U.S.C. 3025(c)(15)), by striking …
//! > (ii) in section 112(c)(1), by striking …
//! > (iii) in section 313(1) (50 U.S.C. 3079(1)), by striking …
//!
//! Or one line cites several sections: *"Sections 351(a), 352(a), 353(a), and
//! 353(b) of such title are amended by striking …"*.
//!
//! The bill states each as one amendment, and the amendment is not split, so
//! its id does not change. Its address gets one item for each place it acts
//! on, and the matcher tries each item with the item's own quoted words.
//!
//! Every case reads the committed corpus. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::document::DocumentNode;
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::legislature::redesignation::Reason;
use words_to_data::legislature::residue::unlinked_amendments;
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::amendment_address::{AmendmentAddress, ItemAddress, addresses_in};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";

/// A committed public law, as the dataset stores it.
fn committed_law(file: &str, bill_id: &str) -> DocumentNode {
    let xml = std::fs::read_to_string(file).expect("the public law should be committed");
    let document = roxmltree::Document::parse(&xml).expect("the public law should parse");
    let (law, _) = bill_expression(&document, bill_id).expect("the law should read");
    law.root
}

/// The address of the one amendment whose id starts with `id_start`.
fn address_of(law: &DocumentNode, id_start: &str) -> AmendmentAddress {
    let found: Vec<AmendmentAddress> = addresses_in(law)
        .into_iter()
        .filter(|address| address.amendment_id.starts_with(id_start))
        .collect();
    let [one] = &found[..] else {
        panic!(
            "one amendment should start with {id_start}, found {}",
            found.len()
        );
    };
    one.clone()
}

/// An item's address as one string: `/us/usc/t50/s3025(c)(15)`, or the
/// reason it has none.
fn printed(item: &ItemAddress) -> String {
    match &item.section {
        Some(section) => {
            let below: String = item
                .container
                .iter()
                .map(|step| format!("({})", step.number))
                .collect();
            format!("{section}{below}")
        }
        None => format!("none: {:?}", item.unresolved),
    }
}

/// Pub. L. 119-60 § 6402(c)(2)(A): a list under the National Security Act of
/// 1947. Each item names a section of the Act, and the publisher's reference
/// in the item's own words places it in title 50.
#[test]
fn should_give_each_item_the_section_its_reference_places_when_a_list_under_an_act_names_none() {
    let law = committed_law(NDAA, NDAA_ID);
    let address = address_of(&law, "f0261be731d3");

    // The address of the whole instruction does not change.
    assert_eq!(address.section, None);
    assert_eq!(address.unresolved, Some(Reason::NoSectionNamed));

    let items: Vec<String> = address.items.iter().map(printed).collect();
    assert_eq!(
        items,
        vec![
            "/us/usc/t50/s3025(c)(15)",
            "/us/usc/t50/s3047(c)(1)",
            "/us/usc/t50/s3079(1)",
        ]
    );
    for (item, number) in address.items.iter().zip(["i", "ii", "iii"]) {
        assert!(
            item.path.starts_with(&format!("{}/", address.path))
                && item.path.ends_with(&format!("_{number}")),
            "each item has its own path under the instruction: {}",
            item.path
        );
    }
}

/// Pub. L. 119-60 § 8402(a): five items under the NOAA Fleet Modernization
/// Act, each *"in section 603 (33 U.S.C. 891a)—"*.
#[test]
fn should_give_an_item_for_each_section_when_every_item_of_the_list_names_one() {
    let law = committed_law(NDAA, NDAA_ID);
    let address = address_of(&law, "331284f0c930");

    let items: Vec<String> = address.items.iter().map(printed).collect();
    assert_eq!(
        items,
        vec![
            "/us/usc/t33/s891a",
            "/us/usc/t33/s891b",
            "/us/usc/t33/s891c",
            "/us/usc/t33/s891f",
            "/us/usc/t33/s891h",
        ]
    );
}

/// Pub. L. 119-60 § 601(c)(4): *"Sections 351(a), 352(a), 353(a), and 353(b)
/// of such title are amended by striking …"*. The line cites four places in
/// three sections. *"such title"* is title 37, which an earlier line names,
/// and this reader does not read *"such title"*, so no item has a section.
/// Each keeps the cited number in its reason and the designations below it,
/// so that the matcher can read the section from the OLRC's table.
#[test]
fn should_give_an_item_for_each_section_a_line_cites_when_it_cites_several() {
    let law = committed_law(NDAA, NDAA_ID);
    let address = address_of(&law, "214ccd5d3dd6");

    let items: Vec<(Option<Reason>, String, String)> = address
        .items
        .iter()
        .map(|item| {
            let below: String = item
                .container
                .iter()
                .map(|step| format!("({})", step.number))
                .collect();
            (item.unresolved.clone(), below, item.path.clone())
        })
        .collect();
    let reason = |number: &str| Some(Reason::NoTitleForSection(number.to_string()));
    assert_eq!(
        items,
        vec![
            (reason("351"), "(a)".to_string(), address.path.clone()),
            (reason("352"), "(a)".to_string(), address.path.clone()),
            (reason("353"), "(a)".to_string(), address.path.clone()),
            (reason("353"), "(b)".to_string(), address.path.clone()),
        ]
    );
}

/// Pub. L. 119-60 § 1411(c): *"Sections 5(a)(2), 6(d)(1), and 6(d)(2) of such
/// Act (50 U.S.C. 98d(a)(2), 98e(d)(1), 98e(d)(2)) are each amended …"*. The
/// publisher marks only the first as a reference, and the parenthesis lists
/// one section of title 50 for each citation, in the same order.
#[test]
fn should_place_each_cited_section_by_its_reference_when_the_line_gives_one_for_each() {
    let law = committed_law(NDAA, NDAA_ID);
    let address = address_of(&law, "7b1a230ccef2");

    let items: Vec<String> = address.items.iter().map(printed).collect();
    assert_eq!(
        items,
        vec![
            "/us/usc/t50/s98d(a)(2)",
            "/us/usc/t50/s98e(d)(1)",
            "/us/usc/t50/s98e(d)(2)",
        ]
    );
}

/// Pub. L. 119-60 § 7347(a): *"Chapter 47 of title 46, United States Code,
/// is amended— (1) by inserting … (2) …"*. No item names a section: the
/// instruction acts on a chapter. Such an instruction is not read here
/// (#434), and its address has no item.
#[test]
fn should_give_no_item_when_no_item_of_the_list_names_a_section() {
    let law = committed_law(NDAA, NDAA_ID);
    let address = address_of(&law, "20c7d5f9b2c6");

    assert_eq!(address.unresolved, Some(Reason::NoSectionNamed));
    assert!(address.items.is_empty(), "{:?}", address.items);
}

/// A line that cites several places in one section is not a list of
/// sections: Pub. L. 119-21 § 70431(a)(5)(C), *"Sections 1202(b)(2),
/// 1202(g)(2)(A), and 1202(j)(1)(A) are each amended …"*, acts in § 1202,
/// and the OLRC's table gives the instruction that section whole (#259).
#[test]
fn should_give_no_item_when_every_section_a_line_cites_is_the_same() {
    let law = committed_law(
        "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml",
        "119-hr-1",
    );
    let address = address_of(&law, "0907f7336a23");

    assert!(address.items.is_empty(), "{:?}", address.items);
}

// --- The matcher: one answer for each item ---------------------------------

/// Pub. L. 119-21 (`119-hr-1`) with title 42 at its three committed release
/// points, and the committed OLRC table stated as links, as
/// `add-classifications` states them.
fn hr_1_in_title_42() -> Dataset<InMemoryStorage> {
    const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
    const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&BillDownload {
            bill_id: "119-hr-1".to_string(),
            bill_xml: read("public_law.xml"),
            bill_metadata_json: read("metadata.json"),
            cosponsors_json: read("cosponsors.json"),
            votes_json: None,
            member_jsons: std::collections::HashMap::new(),
        })
        .expect("the committed law should load");
    for date in RELEASE_POINTS {
        common::add_uslm_xml(
            &mut dataset,
            &format!("tests/test_data/usc/{date}/usc42.xml"),
            date,
        );
    }
    let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
        .expect("the committed table should read");
    let table = ClassificationTable::parse(&html).expect("the committed table should parse");
    let mut paths = SectionPaths::new();
    for date in RELEASE_POINTS {
        let expression = dataset
            .get_expression(&ExpressionId::new(WorkId::new("uscode/title_42"), date))
            .expect("storage should answer")
            .expect("title 42 is held");
        paths.add_work(&expression.root);
    }
    let scope = dataset.scope().expect("the scope should derive");
    for link in classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links {
        dataset.add_link(link).expect("the link should add");
    }
    dataset
}

fn hr_1_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        match_by_evidence(&hr_1_in_title_42())
            .expect("the matcher should run")
            .matches
    })
}

/// Every answer for one amendment, in the order of its items.
fn answers_for<'a>(matches: &'a [AmendmentMatch], id_start: &str) -> Vec<&'a AmendmentMatch> {
    matches
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect()
}

/// The paths each answer is linked to, with the section of the Code below
/// the title: `section_1397aa/subsection_a`.
fn linked_below_the_chapter(answers: &[&AmendmentMatch]) -> Vec<Vec<String>> {
    answers
        .iter()
        .map(|answer| match &answer.outcome {
            Outcome::Linked(linked) => linked
                .paths()
                .iter()
                .map(|path| {
                    let at = path.find("/section_").expect("a change in a section");
                    path[at + 1..].to_string()
                })
                .collect(),
            other => panic!("{} should be linked: {other:?}", answer.address.path),
        })
        .collect()
}

/// Pub. L. 119-21 § 71401(b), *"Title XXI of the Social Security Act (42
/// U.S.C. 1397aa) is amended— (1) in section 2101— … (2) in section
/// 2105(c)(1) … (3) in section 2106(a)(1) …"*. The items name sections of
/// the Act with no reference to the Code. The OLRC's table classifies each
/// item's place: 71401(b)(1) to 42 U.S.C. 1397aa, (b)(2) to 1397ee, (b)(3)
/// to 1397ff. Before #434 no link held these changes.
///
/// 1397aa(a) changed too, and stays without a link: item (1) inserts
/// *"… established in section 2105(h), the purpose"*, and the Code prints
/// the Act's § 2105(h) as the Code's section, so the quoted words do not show
/// in the change. Item (1) has (b) by its quoted words, so elimination does
/// not give it (a) as well.
#[test]
fn should_link_each_item_to_the_changes_in_its_own_section_when_the_olrc_places_each_item() {
    let answers = answers_for(hr_1_matches(), "ee250eb077b2");

    assert_eq!(answers.len(), 3, "one answer for each item");
    assert!(answers.iter().all(|answer| answer.item));
    assert_eq!(
        linked_below_the_chapter(&answers),
        vec![
            vec!["section_1397aa/subsection_b".to_string()],
            vec!["section_1397ee/subsection_c/paragraph_1".to_string()],
            vec!["section_1397ff/subsection_a/paragraph_1".to_string()],
        ]
    );
}

/// Every link of an item says that it is for one item, and where the item
/// is in the bill.
#[test]
fn should_name_the_item_in_the_evidence_when_a_link_is_for_one_item() {
    let answers = answers_for(hr_1_matches(), "ee250eb077b2");
    let links = answers[1].links();

    assert_eq!(links.len(), 1);
    let reasoning = links[0]
        .provenance
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.reasoning.clone())
        .unwrap_or_default();
    assert!(
        reasoning.contains("for the item at publiclawdocument_119-21/")
            && reasoning.contains("section_71401/subsection_b/paragraph_2"),
        "{reasoning}"
    );
}

// --- Residue: one row for each item that is not linked ---------------------

/// One item linked does not take the others off the residue list: a row
/// names an amendment and an item (#434).
#[test]
fn should_list_the_items_no_link_holds_when_one_item_of_the_amendment_is_linked() {
    let mut dataset = hr_1_in_title_42();
    let answers: Vec<AmendmentMatch> = answers_for(hr_1_matches(), "ee250eb077b2")
        .into_iter()
        .cloned()
        .collect();
    for link in answers[0].links() {
        dataset.add_link(link).expect("the link should add");
    }

    let rows: Vec<String> = unlinked_amendments(&dataset, Some("119-hr-1"))
        .expect("residue should derive")
        .into_iter()
        .filter(|row| row.amendment_id.starts_with("ee250eb077b2"))
        .map(|row| {
            assert!(row.item, "each row is for one item");
            row.address
                .path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(rows, vec!["paragraph_2", "paragraph_3"]);
}

/// Pub. L. 119-60 (`119-s-1071`) with title 33 at the release points before
/// and after it.
fn ndaa_matches_in_title_33() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill = parse_bill_amendments(NDAA_ID, NDAA).expect("the law should parse");
        dataset.add_bill(bill).expect("the bill should add");
        let xml = std::fs::read_to_string(NDAA).expect("the law should be committed");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, NDAA_ID).expect("the law should read");
        dataset.add_expression(law).expect("the law should add");
        for date in ["2026-01-23", "2026-07-12"] {
            common::add_uslm_xml(
                &mut dataset,
                &format!("tests/test_data/usc/{date}/usc33.xml"),
                date,
            );
        }
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

/// Pub. L. 119-60 § 8402(a), the five items under the NOAA Fleet
/// Modernization Act: each item's changes are in its own section of title
/// 33.
#[test]
fn should_link_each_item_only_inside_its_own_section_when_the_markup_places_each_item() {
    let answers = answers_for(ndaa_matches_in_title_33(), "331284f0c930");

    assert_eq!(answers.len(), 5, "one answer for each item");
    let linked = linked_below_the_chapter(&answers);
    for (paths, section) in linked.iter().zip(["891a", "891b", "891c", "891f", "891h"]) {
        assert!(!paths.is_empty(), "§ {section} has a link");
        for path in paths {
            assert!(
                path == &format!("section_{section}")
                    || path.starts_with(&format!("section_{section}/")),
                "a change of § {section}'s item is in § {section}: {path}"
            );
        }
    }
}

/// Pub. L. 119-60 § 7701(g)(5): *"Section 1005(b)(5)(B) (33 U.S.C.
/// 2716(b)(5)(B)) is amended by striking …"*. The publisher's reference is
/// wrong: § 1005 of the Oil Pollution Act is 33 U.S.C. 2705. The markup wins,
/// and the item's quoted words do not show in § 2716, so the item gets no
/// link rather than a wrong one.
#[test]
fn should_give_no_link_to_an_item_when_its_reference_places_it_in_the_wrong_section() {
    let answers = answers_for(ndaa_matches_in_title_33(), "c1213b7b65b7");

    let fifth = answers
        .iter()
        .find(|answer| answer.address.path.ends_with("/paragraph_5"))
        .expect("item (5) has an answer");
    assert_eq!(fifth.address.section.as_deref(), Some("/us/usc/t33/s2716"));
    assert!(
        matches!(fifth.outcome, Outcome::Residue(_)),
        "{:?}",
        fifth.outcome
    );
}

/// The words of a level of the law and of every level below it.
fn words_below(node: &DocumentNode) -> String {
    let data = &node.data;
    let mut words: Vec<String> = [
        &data.heading,
        &data.chapeau,
        &data.content,
        &data.proviso,
        &data.continuation,
    ]
    .into_iter()
    .flatten()
    .map(|field| field.to_string())
    .collect();
    words.extend(node.children.iter().map(words_below));
    words.join(" ")
}

/// The quoted strings a link's evidence names: `struck "proposed"`.
fn strings_named(why: &str) -> Vec<String> {
    let mut named = Vec::new();
    for marker in ["struck \"", "inserted \"", "quoted \""] {
        let mut rest = why;
        while let Some(at) = rest.find(marker) {
            rest = &rest[at + marker.len()..];
            let end = rest.find('"').unwrap_or(rest.len());
            named.push(rest[..end].to_string());
        }
    }
    named
}

/// Each item quotes only its own words: every string a link of an item
/// names is quoted in that item's own words, and not only in another item's.
/// Pub. L. 119-60 § 8402(a) and § 7701(g) quote "Secretary", "subsection
/// (d)" and other words in more than one item.
#[test]
fn should_name_only_the_items_own_quoted_words_when_items_of_one_list_quote_different_words() {
    let law = committed_law(NDAA, NDAA_ID);
    let mut checked = 0;
    for id in ["331284f0c930", "c1213b7b65b7"] {
        for answer in answers_for(ndaa_matches_in_title_33(), id) {
            let Outcome::Linked(linked) = &answer.outcome else {
                continue;
            };
            let item = law
                .find(&answer.address.path)
                .expect("the item is in the law");
            let words = words_below(item);
            for change in &linked.changes {
                for string in strings_named(&change.why) {
                    assert!(
                        words.contains(&format!("\"{string}\"")),
                        "{} names {string:?}, which the item at {} does not quote",
                        change.path,
                        answer.address.path
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked > 20,
        "the links name the words they show: {checked}"
    );
}

// --- The command ------------------------------------------------------------

/// `amendment-addresses` prints each item under its amendment, so that a
/// reader of the text sees where the amendment acts. Pub. L. 119-60 §
/// 8402(a) has five items.
#[test]
fn should_print_each_item_under_its_amendment_when_the_command_shows_one_that_acts_on_many_sections()
 {
    let bill = parse_bill_amendments(NDAA_ID, NDAA).expect("the law should parse");
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset.add_bill(bill).expect("the bill should add");
    let xml = std::fs::read_to_string(NDAA).expect("the law should be committed");
    let document = roxmltree::Document::parse(&xml).expect("the law should parse");
    let (law, _) = bill_expression(&document, NDAA_ID).expect("the law should read");
    dataset.add_expression(law).expect("the law should add");
    let path = format!(
        "{}/many_sections_addresses.json",
        env!("CARGO_TARGET_TMPDIR")
    );
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");

    let run = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["amendment-addresses", &path, "--bill", NDAA_ID])
        .args(["--amendment", "331284f0c930"])
        .output()
        .expect("the binary should run");

    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let printed = String::from_utf8_lossy(&run.stdout);
    for section in ["891a", "891b", "891c", "891f", "891h"] {
        assert!(
            printed.contains(&format!("item  /us/usc/t33/s{section}")),
            "{printed}"
        );
    }
}
