use std::collections::{BTreeMap, HashMap};
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
    dataset::{Expression, ExpressionId, WorkId},
    document::DocumentNode,
    io::load_xml_file,
    legislature::{AmendingAction, BillAmendment},
    uslm::parser::{ParseError, normalize_quotes},
    uslm::{AmendmentFacts, UslmFacts},
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
    Ok(bill_of_document(
        &roxmltree::Document::parse(xml_str)?,
        bill_id,
    ))
}

/// The amendments a bill states, from XML already read into memory
///
/// The same extraction as [`parse_bill_amendments_from_str_with_report`], from
/// an XML document a caller has already built, so that one read of the bill can
/// feed the amendments, the document and the redesignations alike
/// (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::bill_of_document;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
///
/// let (bill, report) = bill_of_document(&document, "119-hr-1");
/// assert!(!bill.amendments.is_empty());
/// assert!(report.is_empty());
/// ```
pub fn bill_of_document(document: &roxmltree::Document, bill_id: &str) -> (Bill, AmendmentReport) {
    let (amendments, report) = amendments_with_report(&document.root(), bill_id);
    (
        Bill {
            bill_id: bill_id.to_string(),
            amendments,
        },
        report,
    )
}

/// The date a public law's own markup says it was approved
///
/// A bill is published once and never amended, so this is the one date it has,
/// exactly as a court opinion has the date the court filed it. The bill states
/// it in its `<meta>`, as `<approvedDate>` and again as Dublin Core `<date>`.
///
/// Read from the bill rather than taken from the caller. A date supplied
/// alongside a document is a second place the same fact lives, and after an edit
/// one of the two will be wrong.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::approved_date;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
///
/// assert_eq!(approved_date(&document).unwrap(), "2025-07-04");
/// ```
///
/// # Errors
///
/// Returns [`ParseError::UnsupportedDocumentType`] when the markup states no
/// approval date. A guessed date would put the bill in the wrong place in every
/// dataset that held it, which is the confident wrong answer this project exists
/// to prevent.
pub fn approved_date(document: &roxmltree::Document) -> Result<String> {
    let meta = document
        .descendants()
        .find(|node| node.has_tag_name("meta"))
        .ok_or_else(|| {
            ParseError::UnsupportedDocumentType("the bill states no <meta>".to_string())
        })?;

    meta.children()
        .find(|node| node.has_tag_name("approvedDate") || node.tag_name().name() == "date")
        .and_then(|node| node.text())
        .map(str::trim)
        .filter(|date| !date.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            ParseError::UnsupportedDocumentType(
                "the bill states no date it was approved".to_string(),
            )
        })
}

/// The bill as one expression of one work, ready to store
///
/// A bill is a Work with an Expression, in the same collection as the Code, and
/// a court opinion already enters a dataset this way. The work is the root
/// node's own path — `publiclawdocument_119-21`, built from the number the
/// publisher gave the law — and the date is the day the bill says it was
/// approved (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).
///
/// `bill_id` is the name the dataset knows the bill by, such as `119-hr-1`. It
/// is not the publisher's number, and it is the one the amendment hashes and
/// every link are minted under, so the stored document states its amendments
/// under the same name the rest of the dataset uses.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::bill_expression;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
///
/// let (expression, _) = bill_expression(&document, "119-hr-1").unwrap();
/// assert_eq!(expression.id.to_string(), "publiclawdocument_119-21@2025-07-04");
/// ```
pub fn bill_expression(
    document: &roxmltree::Document,
    bill_id: &str,
) -> Result<(Expression, crate::uslm::parser::ParseReport)> {
    let date = approved_date(document)?;
    let (mut root, report) = crate::uslm::parser::parse_from_document_with_report(document, &date)?;
    state_amendments(&mut root, document, bill_id)?;
    Ok((
        Expression {
            id: ExpressionId::new(WorkId::new(root.data.path.to_string()), date),
            label: None,
            root,
        },
        report,
    ))
}

/// Record on each instruction node the amendment its words are
///
/// The USLM parser builds a tree for any document class and knows nothing about
/// amendments, so this is a second pass over the same markup rather than an
/// argument threaded through the parser. It costs one walk of a bill, and it
/// keeps the bill's vocabulary in the bill's own module.
///
/// An instruction is matched to its node by the publisher's `identifier`, which
/// is the one name both the markup and the tree hold. An instruction the parser
/// left out of the tree — quoted text, a repealed element — therefore records
/// nothing, which is the honest answer: the dataset has no node to point at.
fn state_amendments(
    root: &mut DocumentNode,
    document: &roxmltree::Document,
    bill_id: &str,
) -> Result<()> {
    let stated: HashMap<&str, AmendmentFacts> = document
        .root()
        .descendants()
        .filter(|node| node.attribute("role").unwrap_or_default() == "instruction")
        .filter_map(|node| {
            Some((
                node.attribute("identifier")?,
                AmendmentFacts {
                    id: compute_amendment_id(bill_id, &node_text(&node)),
                    enacted_text: enacted_text(&node),
                },
            ))
        })
        .collect();

    record_amendments(root, &stated)
}

/// Walk the tree, writing each node's amendment into its class payload.
fn record_amendments(
    node: &mut DocumentNode,
    stated: &HashMap<&str, AmendmentFacts>,
) -> Result<()> {
    if let Some(mut facts) = UslmFacts::of(&node.data) {
        let amendment = facts
            .uslm_id
            .as_deref()
            .and_then(|uslm_id| stated.get(uslm_id));
        if let Some(amendment) = amendment {
            facts.amendment = Some(amendment.clone());
            node.data.payload = Some(facts.to_payload()?);
        }
    }
    for child in &mut node.children {
        record_amendments(child, stated)?;
    }
    Ok(())
}

/// The text an instruction enacts, in document order
///
/// The bill's own `<quotedContent>`, which the parser keeps out of the tree
/// because it is not law in force (#86). One entry for each quoted block.
///
/// A block nested inside another is not taken twice: the outer block already
/// carries the inner one's words, and the inner block is part of what the outer
/// one enacts.
fn enacted_text(instruction: &Node) -> Vec<String> {
    instruction
        .descendants()
        .filter(|node| node.tag_name().name().eq_ignore_ascii_case("quotedContent"))
        .filter(|node| {
            !node.ancestors().skip(1).any(|above| {
                above
                    .tag_name()
                    .name()
                    .eq_ignore_ascii_case("quotedContent")
            })
        })
        .map(|node| node_text(&node))
        .filter(|text| !text.trim().is_empty())
        .collect()
}

/// Where each amendment's words sit in a bill's document, by content id
///
/// The other half of [`AmendmentFacts`]: an amendment is identified by its
/// content hash and located by the path of the node whose words it is
/// (`docs/adr/0001-structural-paths-locate-not-identify.md`).
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::{amendment_paths, bill_expression};
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
/// let (bill, _) = bill_expression(&document, "119-hr-1").unwrap();
///
/// let located = amendment_paths(&bill.root);
/// assert_eq!(located.len(), 603);
/// assert!(located.values().all(|path| path.starts_with("publiclawdocument_119-21/")));
/// ```
pub fn amendment_paths(root: &DocumentNode) -> BTreeMap<String, String> {
    let mut located = BTreeMap::new();
    collect_amendment_paths(root, &mut located);
    located
}

fn collect_amendment_paths(node: &DocumentNode, located: &mut BTreeMap<String, String>) {
    if let Some(amendment) = UslmFacts::of(&node.data).and_then(|facts| facts.amendment) {
        located.insert(amendment.id, node.data.path.to_string());
    }
    for child in &node.children {
        collect_amendment_paths(child, located);
    }
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
