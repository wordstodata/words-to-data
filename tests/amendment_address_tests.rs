//! Which Code address each amending instruction of a public law acts on (#248).
//!
//! The publisher's markup already says where an amendment acts: the section its
//! amending line names, the designations its citation gives below that section,
//! and the containers its scope phrases open (*"in paragraph (2)"*). This is
//! stage 2 of `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//!
//! Every case reads the committed public law `119-hr-1`. Nothing is mocked.

use std::collections::HashMap;
use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::document::DocumentNode;
use words_to_data::uslm::UslmFacts;
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
    let addresses: Vec<AmendmentAddress> = addresses_in(&bill);

    let stated = amendment_paths(&bill);
    assert_eq!(addresses.len(), stated.len());
    assert_eq!(
        stated.len(),
        603,
        "the committed bill states 603 amendments"
    );

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
    let addresses = addresses_in(&bill);

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
    let addresses = addresses_in(&bill);

    let address = address_saying(
        &addresses,
        "Part VI of subchapter B of chapter 1 is amended by inserting after section 174 the following new section",
    );

    assert_eq!(address.section.as_deref(), Some("/us/usc/t26/s174A"));
    assert_eq!(address.unresolved, None);
    assert!(address.container.is_empty());
}

/// The measurement ADR 0013 records: over every amending instruction of
/// `119-hr-1`, the markup resolver named a section for 495 of 603 before the
/// en-dash and new-section fixes. The resolver must answer for at least as
/// many, and the two fixes add at least the two instructions above.
#[test]
fn should_address_more_than_the_measured_share_when_every_instruction_of_the_bill_is_read() {
    let bill = committed_bill();
    let addresses = addresses_in(&bill);

    let addressed = addresses
        .iter()
        .filter(|address| address.section.is_some())
        .count();
    println!("addressed {addressed} of {}", addresses.len());

    assert!(
        addressed > 495,
        "the measured baseline is 495 of 603, found {addressed}"
    );
}

// --- What the prose reader of `section-agreement` had to guess at ----------
//
// Before #248, `section-agreement` read an amendment's section out of its
// words with a prose reader of its own. These are the cases that reader was
// written against, carried over to the resolver that replaced it. The markup
// sets quoted text apart, so none of them needs a guess any more.

/// The clause the bill writes at § 83001(a)(2)(B), which quotes the section it
/// searches **for**:
///
/// > by inserting ", as in effect for such academic year," after
/// > "section 479A(b)(1)(B)(v)"
///
/// § 479A is only the string to search for. The clause sits under the
/// instruction *"Section 401(b)(1)(D) of the Higher Education Act of 1965 (20
/// U.S.C. 1070a(b)(1)(D)) is amended—"*, and the maintainer's dataset carries a
/// link on these words inside § 1070a, where the change landed.
#[test]
fn should_not_address_a_section_the_instruction_only_quotes() {
    let bill = committed_bill();
    let addresses = addresses_in(&bill);

    let address = address_saying(
        &addresses,
        "Section 401(b)(1)(D) of the Higher Education Act of 1965",
    );

    assert_eq!(address.section.as_deref(), Some("/us/usc/t20/s1070a"));
    assert_eq!(step_numbers(&address), ["b", "1", "D"]);
}

/// `Section 1400Z-1(b)` with a hyphen, which the Code numbers as one section.
#[test]
fn should_read_the_whole_section_number_when_the_citation_carries_a_hyphen() {
    let bill = committed_bill();
    let addresses = addresses_in(&bill);

    let address = address_saying(&addresses, "1400Z-1(b) is amended by striking paragraph");

    assert_eq!(address.section.as_deref(), Some("/us/usc/t26/s1400Z-1"));
    assert_eq!(step_numbers(&address), ["b"]);
}

/// > Section 7701(a) is amended by adding at the end the following new
/// > paragraphs:
///
/// The maintainer's dataset carries two links on these words inside § 48E,
/// which only **uses** the new definitions. The address is § 7701(a).
#[test]
fn should_address_the_cited_section_and_its_designations_when_the_line_cites_them() {
    let bill = committed_bill();
    let addresses = addresses_in(&bill);

    let address = address_saying(
        &addresses,
        "Section 7701(a) is amended by adding at the end the following new paragraphs",
    );

    assert_eq!(address.section.as_deref(), Some("/us/usc/t26/s7701"));
    assert_eq!(step_numbers(&address), ["a"]);
}

/// The new § 4968(c) the bill enacts:
///
/// > (c) Applicable Educational Institution.—For purposes of this subchapter,
/// > the term 'applicable educational institution' means an eligible
/// > educational institution (as defined in section 25A(f)(2))—
///
/// § 25A is only where a term is defined. The instruction that enacts this
/// text does not act on § 25A.
#[test]
fn should_not_address_a_section_the_enacted_text_only_cross_references() {
    let bill = committed_bill();
    let addresses = addresses_in(&bill);

    let mut enacting = Vec::new();
    collect_amendments_enacting(&bill, "(as defined in section 25A(f)(2))", &mut enacting);
    assert!(
        !enacting.is_empty(),
        "an instruction of the bill should enact the new § 4968(c)"
    );

    for amendment_id in &enacting {
        let address = addresses
            .iter()
            .find(|address| &address.amendment_id == amendment_id)
            .expect("every amendment has an address or a reason");
        assert_ne!(
            address.section.as_deref(),
            Some("/us/usc/t26/s25A"),
            "§ 25A is a cross-reference in the enacted text: {address:?}"
        );
    }
}

/// The ids of the amendments whose enacted text holds `phrase`.
fn collect_amendments_enacting(node: &DocumentNode, phrase: &str, found: &mut Vec<String>) {
    if let Some(amendment) = UslmFacts::of(&node.data).and_then(|facts| facts.amendment)
        && amendment
            .enacted_text
            .iter()
            .any(|block| block.contains(phrase))
    {
        found.push(amendment.id);
    }
    for child in &node.children {
        collect_amendments_enacting(child, phrase, found);
    }
}

// --- The command ----------------------------------------------------------

/// The committed bill as the Congress client would hand it over.
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
        member_jsons: HashMap::new(),
    }
}

/// A dataset file that holds the committed bill and nothing else, saved under
/// `name` in the test target's own directory.
fn dataset_holding_the_bill(name: &str) -> String {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");
    path
}

/// Run the command, and read what it printed as JSON.
fn run_amendment_addresses(args: &[&str]) -> serde_json::Value {
    let run = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg("amendment-addresses")
        .args(args)
        .output()
        .expect("the binary should run");
    assert!(
        run.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    serde_json::from_slice(&run.stdout).expect("--json should emit json")
}

/// The command shows every amendment of a public law, with its address or
/// its reason. `--json` is the surface an agent reads.
#[test]
fn should_show_every_amendments_address_or_reason_when_the_command_runs_with_json() {
    let path = dataset_holding_the_bill("amendment_addresses_all.json");

    let shown = run_amendment_addresses(&[&path, "--bill", BILL_ID, "--json"]);

    let rows = shown.as_array().expect("an array of addresses");
    assert_eq!(rows.len(), 603);
    let new_section = rows
        .iter()
        .find(|row| row["section"] == "/us/usc/t26/s174A")
        .expect("the new § 174A is one of the addresses");
    assert!(new_section["amendment_id"].is_string());
    assert!(
        rows.iter()
            .all(|row| row["section"].is_null() != row["unresolved"].is_null()),
        "every row has a section or a reason"
    );
}

/// `--amendment` narrows the command to one amendment, by the start of its id,
/// which is how `settle` and the review queue name things.
#[test]
fn should_show_only_that_amendment_when_the_command_is_given_its_id() {
    let bill = committed_bill();
    let addresses = addresses_in(&bill);
    let new_section = addresses
        .iter()
        .find(|address| address.section.as_deref() == Some("/us/usc/t26/s174A"))
        .expect("the bill inserts § 174A");
    let short_id = &new_section.amendment_id[..12];

    let path = dataset_holding_the_bill("amendment_addresses_one.json");
    let shown =
        run_amendment_addresses(&[&path, "--bill", BILL_ID, "--amendment", short_id, "--json"]);

    let rows = shown.as_array().expect("an array of addresses");
    assert_eq!(rows.len(), 1, "one amendment was asked for, found {rows:?}");
    assert_eq!(rows[0]["amendment_id"], new_section.amendment_id.as_str());
    assert_eq!(rows[0]["section"], "/us/usc/t26/s174A");
}
