use std::str::FromStr;

/// Bill-specific parsing logic
///
/// This module handles parsing Public Laws and extracting bill-specific information:
/// - Amending actions that reference USC sections
/// - Quoted content that represents new USC text
use roxmltree::Node;

use crate::{
    io::load_xml_file,
    uslm::{
        AmendingAction, BillAmendment, ElementType,
        parser::{ParseError, extract_number},
    },
};

/// Data extracted from a bill document
///
/// Contains the bill identifier and all amendments found within the bill
/// that modify the United States Code.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AmendmentData {
    /// The bill identifier (e.g., "119-21" for the 119th Congress, 21st law)
    pub bill_id: String,

    /// All amendments extracted from instruction elements in the bill
    pub amendments: Vec<BillAmendment>,
}

pub type Result<T> = std::result::Result<T, ParseError>;

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
/// * `xml_str` - The Public Law XML content as a string
///
/// # Returns
///
/// An `AmendmentData` struct containing the bill ID and all extracted amendments,
/// or a `ParseError` if parsing fails.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::uslm::bill_parser::parse_bill_amendments_from_str;
///
/// let xml = std::fs::read_to_string("bill.xml").unwrap();
/// let data = parse_bill_amendments_from_str(&xml).unwrap();
/// ```
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The XML is malformed
/// - No `<pLaw>` element is found
/// - Required elements are missing from the XML structure
pub fn parse_bill_amendments_from_str(xml_str: &str) -> Result<AmendmentData> {
    let doc = roxmltree::Document::parse(xml_str)?;
    let plaw_node = doc.descendants().find(|n| n.has_tag_name("pLaw"));
    match plaw_node {
        None => Err(ParseError::UnableToParseElement(
            "Could not find public law document".to_string(),
        )),
        Some(node) => {
            // Extract bill_id
            let element_type = ElementType::from_str(node.tag_name().name())?;
            let number = extract_number(element_type, &node)?;
            let amendments = get_amendments(&node);
            Ok(AmendmentData {
                bill_id: number.value,
                amendments,
            })
        }
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
/// * `path` - Path to the Public Law XML file
///
/// # Returns
///
/// An `AmendmentData` struct containing the bill ID and all extracted amendments,
/// or a `ParseError` if parsing fails.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::parse_bill_amendments;
///
/// let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();
/// assert_eq!(data.bill_id, "119-21");
/// assert!(!data.amendments.is_empty());
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
pub fn parse_bill_amendments(path: &str) -> Result<AmendmentData> {
    let xml_str = load_xml_file(path)?;
    parse_bill_amendments_from_str(&xml_str)
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
///
/// # Returns
///
/// A vector of `BillAmendment` structures, one for each instruction element found.
/// Each amendment contains:
/// - Amending action types from `<amendingAction>` tags
/// - The full readable text of the instruction element
///
/// # Implementation Note
///
/// This is a naive and simple extraction approach. A more sophisticated
/// implementation could better handle complex legislative language patterns,
/// nested instructions, and implicit amendments.
///
/// # Examples
///
/// ```no_run
/// use roxmltree::Document;
/// use words_to_data::uslm::bill_parser::get_amendments;
///
/// let xml = std::fs::read_to_string("bill.xml").unwrap();
/// let doc = Document::parse(&xml).unwrap();
/// let root = doc.root_element();
/// let amendments = get_amendments(&root);
/// ```
pub fn get_amendments(node: &Node) -> Vec<BillAmendment> {
    let nodes = node
        .descendants()
        .filter(|p| p.attribute("role").unwrap_or_default() == "instruction");
    nodes.map(|n| get_amendment_data(&n)).collect()
}

fn get_amendment_data(node: &Node) -> BillAmendment {
    let mut action_types: Vec<AmendingAction> = Vec::new();

    // Find all <amendingAction> tags
    for descendant in node.descendants() {
        if descendant.tag_name().name().to_lowercase().as_str() == "amendingaction" {
            let action_text = descendant
                .attribute("type")
                .expect("I expect that Amending Action tags are never empty, so I'll be surprised if this ever fails");
            if let Ok(action) = AmendingAction::from_str(action_text) {
                action_types.push(action);
            }
        }
    }

    BillAmendment {
        action_types,
        amending_text: node_text(node),
    }
}

fn node_text(node: &Node) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .map(|n| n.text().unwrap_or(""))
        .collect()
}
