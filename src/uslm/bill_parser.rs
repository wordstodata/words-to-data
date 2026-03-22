use std::str::FromStr;

/// Bill-specific parsing logic
///
/// This module handles parsing Public Laws and extracting bill-specific information:
/// - Amending actions that reference USC sections
/// - USC references from <ref> tags
/// - Quoted content that represents new USC text
use roxmltree::Node;

use crate::{
    io::load_xml_file,
    uslm::{
        AmendingAction, BillAmendment, ElementType, UscReference,
        parser::{ParseError, extract_number},
    },
};

/// Data extracted from a bill document
///
/// Contains the bill identifier and all amendments found within the bill
/// that modify the United States Code.
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
/// - The source path (bill location) from the `identifier` attribute
/// - Target USC paths referenced in `<ref>` tags
/// - Amending action types from `<amendingAction>` tags
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
    nodes
        .map(|n| {
            let source_path = n.attribute("identifier").unwrap();
            get_amendment_data(&n, source_path)
        })
        .collect()
}

fn get_amendment_data(node: &Node, source_path: &str) -> BillAmendment {
    let mut target_paths: Vec<UscReference> = Vec::new();
    let mut action_types: Vec<AmendingAction> = Vec::new();

    // Find all <ref> and <amendingAction <tags>
    for descendant in node.descendants() {
        match descendant.tag_name().name().to_lowercase().as_str() {
            // <ref> tags tell you the USLM path to the target law
            "ref" => {
                if let Some(href) = descendant.attribute("href") {
                    // Check if this is a USC reference
                    if href.starts_with("/us/usc/") {
                        let display_text = descendant
                            .text()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| href.to_string());

                        target_paths.push(UscReference {
                            path: href.to_string(),
                            display_text,
                        });
                    }
                }
            }
            "amendingaction" => {
                let action_text = descendant
                    .attribute("type")
                    .expect("I expect that Amending Action tags are never empty, so I'll be surprised if this ever fails");
                if let Ok(action) = AmendingAction::from_str(action_text) {
                    action_types.push(action);
                }
            }
            _ => {}
        }
    }

    BillAmendment {
        source_path: source_path.to_string(),
        target_paths,
        action_types,
    }
}
