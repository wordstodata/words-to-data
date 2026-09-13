use std::collections::HashMap;
use std::str::FromStr;

/// Bill-specific parsing logic
///
/// This module handles parsing Public Laws and extracting bill-specific information:
/// - Amending actions that reference USC sections
/// - Quoted content that represents new USC text
use hex;
use roxmltree::Node;
use sha2::{Digest, Sha256};

use crate::{
    io::load_xml_file,
    legislature::{AmendingAction, BillAmendment},
    uslm::parser::{ParseError, normalize_quotes},
};

/// Data extracted from a bill document
///
/// Contains the bill identifier and all amendments found within the bill
/// that modify the United States Code.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Bill {
    /// The bill identifier (e.g., "119-21" for the 119th Congress, 21st law)
    pub bill_id: String,

    /// All amendments extracted from instruction elements in the bill
    /// Keyed by content-based ID: sha256("{bill_id}:{amending_text}")
    pub amendments: HashMap<String, BillAmendment>,
}

pub type Result<T> = std::result::Result<T, ParseError>;

/// An `amendingAction/@type` this build does not know.
///
/// Enough to act on: the word the bill wrote, and where in the bill it wrote it.
/// A count on its own would not do, because the fix depends on which action the
/// publisher used and on what that instruction says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadAmendingAction {
    /// The `type` attribute, verbatim, such as `substitute`.
    pub value: String,
    /// Where the instruction that held it sits: the bill's own `identifier`,
    /// such as `/us/pl/119/21/tI/stA/s10101/a`. An instruction that carries no
    /// identifier gives the amendment's content id instead, which finds it in
    /// [`Bill::amendments`].
    pub location: String,
}

/// What one bill parse could not read.
///
/// The publisher's schema allows twelve amending actions and this build reads all
/// twelve, so an entry here means the bill wrote something else. That must be
/// said out loud rather than dropped: silence cannot be told apart from "this
/// amendment names no action". It is the rule #110 set for unknown elements, and
/// [`crate::uslm::parser::ParseReport`] and [`crate::citation::usc::FindReport`]
/// report their own gaps the same way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AmendmentReport {
    /// Every action type declined, in the order the parse met them.
    pub unread_actions: Vec<UnreadAmendingAction>,
}

impl AmendmentReport {
    /// True when every amending action in the bill was read.
    pub fn is_empty(&self) -> bool {
        self.unread_actions.is_empty()
    }

    /// One line for each value met, in the order the values were first met.
    ///
    /// Not one line for each occurrence. A single public law writes over a
    /// thousand `insert` actions, so a value new to this build would arrive in
    /// the thousands too, and a thousand lines is not a report anybody reads.
    /// The line names the value, how many actions it accounts for, and the first
    /// place it was written, so a reader can go and look at one.
    pub fn summary(&self) -> Vec<String> {
        let mut lines = Vec::new();

        for (at, unread) in self.unread_actions.iter().enumerate() {
            let met_before = self.unread_actions[..at]
                .iter()
                .any(|earlier| earlier.value == unread.value);
            if met_before {
                continue;
            }

            let count = self
                .unread_actions
                .iter()
                .filter(|other| other.value == unread.value)
                .count();
            lines.push(format!(
                "warning: {count} amending action(s) of type {:?} were not read. The first is in {}",
                unread.value, unread.location
            ));
        }

        lines
    }

    /// Write the report to stderr, so it reaches the person who ran the command.
    ///
    /// The crate carries no logger, and the USLM parser and the citation finder
    /// both report what they declined with `eprintln!`, so this does the same.
    pub fn print_to_stderr(&self) {
        for line in self.summary() {
            eprintln!("{line}");
        }
    }
}

/// Parse a bill XML string and extract all amendments to the United States Code
///
/// This function parses a Public Law (bill) document from an XML string and extracts
/// structured information about how the bill amends existing USC sections. It identifies:
/// - USC sections being modified (from `<ref>` tags)
/// - The type of amending actions (amend, add, delete, insert, etc.)
/// - The location in the bill where each amendment occurs
///
/// This variant enables unit testing without filesystem access and in-memory
/// parsing workflows.
///
/// # Arguments
///
/// * `bill_id` - The bill identifier (e.g., "119-21" for the 119th Congress, 21st law)
/// * `xml_str` - The Public Law XML content as a string
///
/// # Returns
///
/// A `Bill` struct containing the bill ID and all extracted amendments,
/// or a `ParseError` if parsing fails.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::uslm::bill_parser::parse_bill_amendments_from_str;
///
/// let xml = std::fs::read_to_string("bill.xml").unwrap();
/// let bill = parse_bill_amendments_from_str("119-21", &xml).unwrap();
/// ```
///
/// # Errors
///
/// Returns `ParseError` if the XML is malformed.
///
/// An amending action this build cannot read goes to stderr. Use
/// [`parse_bill_amendments_from_str_with_report`] when the caller decides how one
/// is shown.
pub fn parse_bill_amendments_from_str(bill_id: &str, xml_str: &str) -> Result<Bill> {
    let (bill, report) = parse_bill_amendments_from_str_with_report(bill_id, xml_str)?;
    report.print_to_stderr();
    Ok(bill)
}

/// Parse a bill XML string, and keep the report of what the parse could not read
///
/// The same parse as [`parse_bill_amendments_from_str`], except that the caller
/// receives the [`AmendmentReport`] instead of having it printed to stderr.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::parse_bill_amendments_from_str_with_report;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let (bill, report) = parse_bill_amendments_from_str_with_report("119-21", &xml).unwrap();
///
/// // 119-21 writes six of the schema's twelve actions, and all six are read.
/// assert!(!bill.amendments.is_empty());
/// assert!(report.is_empty());
/// ```
pub fn parse_bill_amendments_from_str_with_report(
    bill_id: &str,
    xml_str: &str,
) -> Result<(Bill, AmendmentReport)> {
    let doc = roxmltree::Document::parse(xml_str)?;
    let (amendments, report) = amendments_with_report(&doc.root(), bill_id);
    Ok((
        Bill {
            bill_id: bill_id.to_string(),
            amendments,
        },
        report,
    ))
}

/// Parse a bill XML file and extract all amendments to the United States Code
///
/// This function parses a Public Law (bill) document and extracts structured
/// information about how the bill amends existing USC sections. It identifies:
/// - USC sections being modified (from `<ref>` tags)
/// - The type of amending actions (amend, add, delete, insert, etc.)
/// - The location in the bill where each amendment occurs
///
/// For in-memory parsing without filesystem access, use `parse_bill_amendments_from_str()` instead.
///
/// # Arguments
///
/// * `bill_id` - The bill identifier (e.g., "119-21" for the 119th Congress, 21st law)
/// * `path` - Path to the Public Law XML file
///
/// # Returns
///
/// A `Bill` struct containing the bill ID and all extracted amendments,
/// or a `ParseError` if parsing fails.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::parse_bill_amendments;
///
/// let bill = parse_bill_amendments("119-21", "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml").unwrap();
/// assert_eq!(bill.bill_id, "119-21");
/// assert!(!bill.amendments.is_empty());
/// ```
///
/// # Amendment Extraction
///
/// The function looks for elements with `role="instruction"` attribute, which
/// typically contain the legislative instructions for amending existing law.
/// Within these elements, it extracts:
///
/// - **USC References**: From `<ref href="/us/usc/...">` tags
/// - **Amending Actions**: From `<amendingAction type="...">` tags
///
/// # Limitations
///
/// This is a simplified amendment extraction. The parser uses a naive approach
/// that may not capture all nuances of complex legislative language. Future
/// versions may implement more sophisticated bill parsing logic.
pub fn parse_bill_amendments(bill_id: &str, path: &str) -> Result<Bill> {
    let xml_str = load_xml_file(path)?;
    parse_bill_amendments_from_str(bill_id, &xml_str)
}

/// Parse a bill XML file, and keep the report of what the parse could not read
///
/// The same parse as [`parse_bill_amendments`], except that the caller receives
/// the [`AmendmentReport`] instead of having it printed to stderr.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::parse_bill_amendments_with_report;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let (bill, report) = parse_bill_amendments_with_report("119-21", path).unwrap();
///
/// assert_eq!(bill.bill_id, "119-21");
/// assert!(report.is_empty());
/// ```
pub fn parse_bill_amendments_with_report(
    bill_id: &str,
    path: &str,
) -> Result<(Bill, AmendmentReport)> {
    let xml_str = load_xml_file(path)?;
    parse_bill_amendments_from_str_with_report(bill_id, &xml_str)
}

/// Compute a content-based amendment ID
///
/// The ID is a SHA256 hash of "{bill_id}:{amending_text}", providing a stable,
/// deterministic identifier that works regardless of the source format (USLM XML,
/// plaintext, etc.).
pub(crate) fn compute_amendment_id(bill_id: &str, amending_text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", bill_id, amending_text));
    let result = hasher.finalize();
    hex::encode(result)
}

/// Extract amendments from a bill XML node
///
/// This function performs a simple extraction of amendments by finding all
/// descendant elements with the `role="instruction"` attribute. Many bills
/// organize their amending language by wrapping instruction elements around
/// the text that modifies existing law.
///
/// # Arguments
///
/// * `node` - The root XML node to search for amendments (typically the bill root)
/// * `bill_id` - The bill identifier, used to compute content-based amendment IDs
///
/// # Returns
///
/// A HashMap of `BillAmendment` structures keyed by content-based ID.
/// Each amendment contains:
/// - A content-based ID (sha256 of bill_id + amending_text)
/// - Amending action types from `<amendingAction>` tags
/// - The full readable text of the instruction element
///
/// # Implementation Note
///
/// This is a naive and simple extraction approach. A more sophisticated
/// implementation could better handle complex legislative language patterns,
/// nested instructions, and implicit amendments.
///
/// An amending action this build cannot read goes to stderr, as an
/// [`AmendmentReport`].
pub fn get_amendments(node: &Node, bill_id: &str) -> HashMap<String, BillAmendment> {
    let (amendments, report) = amendments_with_report(node, bill_id);
    report.print_to_stderr();
    amendments
}

/// The same extraction as [`get_amendments`], keeping the report for the caller.
fn amendments_with_report(
    node: &Node,
    bill_id: &str,
) -> (HashMap<String, BillAmendment>, AmendmentReport) {
    let nodes = node
        .descendants()
        .filter(|p| p.attribute("role").unwrap_or_default() == "instruction");

    let mut amendments = HashMap::new();
    let mut report = AmendmentReport::default();
    for n in nodes {
        let amendment = get_amendment_data(&n, bill_id, &mut report);
        amendments.insert(amendment.id.clone(), amendment);
    }
    (amendments, report)
}

fn get_amendment_data(node: &Node, bill_id: &str, report: &mut AmendmentReport) -> BillAmendment {
    let mut action_types: Vec<AmendingAction> = Vec::new();

    // Find all <amendingAction> tags
    for descendant in node.descendants() {
        if descendant.tag_name().name().to_lowercase().as_str() == "amendingaction" {
            let action_text = descendant
                .attribute("type")
                .expect("I expect that Amending Action tags are never empty, so I'll be surprised if this ever fails");
            match AmendingAction::from_str(action_text) {
                Ok(action) => action_types.push(action),
                // The publisher may add a value, and a bill may simply be wrong.
                // Either way the word is kept and named rather than dropped, so
                // that the next bill to carry one says so instead of quietly
                // losing an action.
                Err(_) => report.unread_actions.push(UnreadAmendingAction {
                    value: action_text.to_string(),
                    location: instruction_location(node, bill_id),
                }),
            }
        }
    }

    let amending_text = node_text(node);
    let id = compute_amendment_id(bill_id, &amending_text);

    BillAmendment {
        // Parsed from the bill, so nothing has been asserted about `changes` yet.
        provenance: None,
        id,
        action_types,
        amending_text,
        changes: Vec::new(),
    }
}

/// Where an instruction sits, for a reader who has to go and look at it.
///
/// A bill numbers its own instructions, so `identifier` points straight at one:
/// `/us/pl/119/21/tI/stA/s10101/a`. Without it the best pointer left is the
/// amendment's content id, which finds the amendment in [`Bill::amendments`].
fn instruction_location(node: &Node, bill_id: &str) -> String {
    node.attribute("identifier")
        .map(str::to_string)
        .unwrap_or_else(|| compute_amendment_id(bill_id, &node_text(node)))
}

pub(crate) fn node_text(node: &Node) -> String {
    let raw: String = node
        .descendants()
        .filter(|n| n.is_text())
        .map(|n| n.text().unwrap_or(""))
        .collect();
    normalize_quotes(&raw)
}
