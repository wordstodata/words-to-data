//! An amendment whose address is inside a provision that a window added or
//! removed whole (#336).
//!
//! The diff lists such a provision as one path, and no path below it. The
//! evidence method links the provision to an amendment that only adds, when
//! the provision was added. Every other such amendment is work.
//!
//! Every tree is from the committed corpus: the public law `119-hr-1`, and
//! titles 7 and 26 of the Code at 2025-07-18 and 2025-07-30. The corpus has
//! no amendment of the law whose address is inside a removed provision, or
//! one that strikes inside an added provision. For those cases the two
//! published trees are stored in the opposite order: the 30 July tree under
//! 2025-07-18 and the 18 July tree under 2025-07-30. Then each provision the
//! law added is removed in the window, and each provision it removed is
//! added. The trees and the bill are real; only their order is changed.

use std::sync::OnceLock;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::legislature::evidence_matching::{
    AmendmentMatch, EvidenceMatching, Outcome, Stage, match_by_evidence,
};
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::parser::parse;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";
const TITLES: [(&str, &str); 2] = [
    ("uscode/title_7", "usc07.xml"),
    ("uscode/title_26", "usc26.xml"),
];

/// Section 70204(a)(1) of the law: "Subchapter F of chapter 1 is amended by
/// adding at the end the following new part: PART IX ... § 530A". The markup
/// names no section, and the OLRC classifies it to 26 U.S.C. 530A.
const ADDS_PART_IX: &str = "7bdf27afebe2";
const PART_IX: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_F/part_IX";

/// Section 70311(b)(3) of the law: "Section 951A(f)(1)(A) is amended by
/// striking "904(h)(1)" and inserting "904(h)"." The law removed § 951A(f)
/// in the same window, so in the reversed order that subsection is added.
const STRIKES_IN_951A_F: &str = "d6ad2f989ef5";
const SECTION_951A_F: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_III/subpart_F/section_951A/subsection_f";

fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// The bill, and titles 7 and 26 stored as `(published, keyed)` pairs: the
/// release point a tree was published at, and the date it is stored under.
/// The OLRC's classification table is stated as links, as
/// `add-classifications` states them.
fn matched_over(trees: [(&str, &str); 2]) -> EvidenceMatching {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    for (work, file) in TITLES {
        for (published, keyed) in trees {
            let parsed = parse(&format!("tests/test_data/usc/{published}/{file}"), keyed)
                .expect("the corpus should parse");
            let root = work_roots(parsed).pop().expect("the file holds one title");
            dataset
                .add_expression(Expression {
                    id: ExpressionId::new(WorkId::new(work), keyed),
                    label: None,
                    root,
                })
                .expect("the tree should store");
        }
    }
    for link in olrc_links(&dataset) {
        dataset.add_link(link).expect("the link should add");
    }
    match_by_evidence(&dataset).expect("the matcher should run")
}

fn olrc_links(dataset: &Dataset<InMemoryStorage>) -> Vec<words_to_data::link::Link> {
    let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
        .expect("the committed table should read");
    let table = ClassificationTable::parse(&html).expect("the committed table should parse");
    let mut paths = SectionPaths::new();
    for (work, _) in TITLES {
        for date in [EARLY, LATE] {
            let expression = dataset
                .get_expression(&ExpressionId::new(WorkId::new(work), date))
                .expect("storage should answer")
                .expect("the title is held");
            paths.add_work(&expression.root);
        }
    }
    let scope = dataset.scope().expect("the scope should derive");
    classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links
}

/// The trees in the order they were published.
fn as_published() -> &'static EvidenceMatching {
    static MATCHED: OnceLock<EvidenceMatching> = OnceLock::new();
    MATCHED.get_or_init(|| matched_over([(EARLY, EARLY), (LATE, LATE)]))
}

/// The trees in the opposite order.
fn reversed() -> &'static EvidenceMatching {
    static MATCHED: OnceLock<EvidenceMatching> = OnceLock::new();
    MATCHED.get_or_init(|| matched_over([(LATE, EARLY), (EARLY, LATE)]))
}

fn answer_for<'a>(matched: &'a EvidenceMatching, id_start: &str) -> &'a AmendmentMatch {
    let found: Vec<&AmendmentMatch> = matched
        .matches
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    let [one] = found[..] else {
        panic!("one answer for {id_start}, found {}", found.len());
    };
    one
}

#[test]
fn should_link_an_amendment_that_only_adds_to_the_added_provision_that_holds_its_address() {
    let answer = answer_for(as_published(), ADDS_PART_IX);

    let Outcome::Linked(linked) = &answer.outcome else {
        panic!("the amendment should link, got {:?}", answer.outcome);
    };
    let paths: Vec<&str> = linked.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, [PART_IX]);
}

#[test]
fn should_list_as_work_an_amendment_whose_address_is_inside_a_removed_provision() {
    // Reversed, the window removes part IX. An amendment that adds § 530A
    // did not remove the part that holds it.
    let answer = answer_for(reversed(), ADDS_PART_IX);

    let Outcome::Residue(residue) = &answer.outcome else {
        panic!("the amendment should not link, got {:?}", answer.outcome);
    };
    assert_eq!(residue.stage, Stage::Window);
    assert!(!residue.quiet, "the address changed: {}", residue.reason);
    assert_eq!(residue.changes, [PART_IX]);
    assert!(
        residue.reason.contains(&format!("{PART_IX} (removed)")),
        "the reason should name the removed part: {}",
        residue.reason
    );
}

#[test]
fn should_list_as_work_an_amendment_that_strikes_inside_an_added_provision() {
    // Reversed, the window adds § 951A(f). Words struck inside it are not
    // the change that added it.
    let answer = answer_for(reversed(), STRIKES_IN_951A_F);

    let Outcome::Residue(residue) = &answer.outcome else {
        panic!("the amendment should not link, got {:?}", answer.outcome);
    };
    assert_eq!(residue.stage, Stage::Window);
    assert!(!residue.quiet, "the address changed: {}", residue.reason);
    assert_eq!(residue.changes, [SECTION_951A_F]);
}
