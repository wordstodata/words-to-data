//! Which Code address each amending instruction of a public law acts on (#248).
//!
//! The publisher's markup already says where an amendment acts: the section its
//! amending line names, the designations its citation gives below that section,
//! and the containers its scope phrases open (*"in paragraph (2)"*). This is
//! stage 2 of `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//!
//! Every case reads the committed public law `119-hr-1`. Nothing is mocked.

use std::collections::HashMap;

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

/// The numbers of an address's steps, outermost first.
fn step_numbers(address: &AmendmentAddress) -> Vec<&str> {
    address
        .container
        .iter()
        .map(|step| step.number.as_str())
        .collect()
}

/// The bill writes `Section 1400Z–2(d)(2)(D)(ii)` with an en dash, as the Code
/// prints it. Stopping at the dash names § 1400Z, a real but different section
/// (#135, #141).
#[test]
fn should_read_the_whole_section_number_when_the_citation_carries_an_en_dash() {
    let bill = committed_bill();
    let addresses = addresses_in(BILL_ID, &bill);

    let address = address_saying(&addresses, "Section 1400Z–2(d)(2)(D)(ii)");

    assert_eq!(address.section.as_deref(), Some("/us/usc/t26/s1400Z-2"));
    assert_eq!(step_numbers(&address), ["d", "2", "D", "ii"]);
}

/// An amendment that inserts a whole new section names a *part* in its
/// amending line:
///
/// > Part VI of subchapter B of chapter 1 is amended by inserting after section
/// > 174 the following new section:"SEC. 174A. 26 USC 174A. …
///
/// § 174 is only the anchor. The new section's own `SEC. 174A.` heading and
/// the publisher's marginal note `26 USC 174A` both name the address.
#[test]
fn should_address_the_new_section_when_an_amendment_inserts_one() {
    let bill = committed_bill();
    let addresses = addresses_in(BILL_ID, &bill);

    let address = address_saying(
        &addresses,
        "Part VI of subchapter B of chapter 1 is amended by inserting after section 174 the following new section",
    );

    assert_eq!(address.section.as_deref(), Some("/us/usc/t26/s174A"));
    assert_eq!(address.unresolved, None);
    assert!(address.container.is_empty());
}
