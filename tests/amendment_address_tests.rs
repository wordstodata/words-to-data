//! Which Code address each amending instruction of a public law acts on (#248).
//!
//! The publisher's markup already says where an amendment acts: the section its
//! amending line names, the designations its citation gives below that section,
//! and the containers its scope phrases open (*"in paragraph (2)"*). This is
//! stage 2 of `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//!
//! Every case reads the committed public law `119-hr-1`. Nothing is mocked.

use std::collections::HashMap;

use words_to_data::congress::BillDownload;
use words_to_data::document::DocumentNode;
use words_to_data::uslm::amendment_address::{AmendmentAddress, addresses_in};
use words_to_data::uslm::bill_parser::{amendment_paths, bill_expression};

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// The committed bill as the dataset stores it: one document, read once.
fn committed_bill() -> DocumentNode {
    let xml = std::fs::read_to_string(format!("{BILL_DIR}/public_law.xml"))
        .expect("the public law should be committed");
    let document = roxmltree::Document::parse(&xml).expect("the public law should parse");
    let (bill, _) = bill_expression(&document, BILL_ID).expect("the bill should read");
    bill.root
}

/// Every amending instruction comes back, addressed or with its reason.
///
/// An instruction the resolver drops would make its silence read as the bill's
/// silence, which is the rule #110 set for unknown elements.
#[test]
fn should_return_every_amendment_with_an_address_or_a_reason_when_a_public_law_is_read() {
    let bill = committed_bill();
    let addresses: Vec<AmendmentAddress> = addresses_in(BILL_ID, &bill);

    let stated = amendment_paths(&bill);
    assert_eq!(addresses.len(), stated.len());
    assert_eq!(stated.len(), 603, "the committed bill states 603 amendments");

    let by_id: HashMap<&str, &AmendmentAddress> = addresses
        .iter()
        .map(|address| (address.amendment_id.as_str(), address))
        .collect();
    assert_eq!(by_id.len(), 603, "each amendment is answered once");

    for address in &addresses {
        assert!(
            address.section.is_some() != address.unresolved.is_some(),
            "an amendment has either a section or a reason, never both or \
             neither: {address:?}"
        );
    }
}
