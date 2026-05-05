use std::str::FromStr;

use thiserror::Error;

use crate::{
    io::load_xml_file,
    uslm::{
        self, BillType, DocumentType, ElementData, ElementType, RefPair, SourceCredit, USCType,
        USLMElement, USLMError, path::should_include_in_uslm_path,
    },
};

// Re-export path functions for backward compatibility with existing API
pub use crate::uslm::path::generate_structural_path;

/// Errors that can occur during USLM XML parsing
#[derive(Error, Debug)]
pub enum ParseError {
    /// XML structure is malformed or invalid
    #[error("XML parsing error: {0}")]
    Xml(#[from] roxmltree::Error),

    /// File I/O error (file not found, permission denied, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// File contains invalid UTF-8 encoding
    #[error("Invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// USLM-specific data error
    #[error("USLM Error")]
    USLMDataError(#[from] USLMError),

    /// The document type is not recognized or supported
    #[error("Unsupported Document Type {0}")]
    UnsupportedDocumentType(String),

    /// Date string is malformed or represents an invalid date
    #[error("Invalid Date")]
    InvalidDate,

    /// JSON serialization failed
    #[error("Serialization Error")]
    SerializationError(#[from] serde_json::Error),

    /// An element could not be parsed from the XML
    #[error("Unable to parse element {0}")]
    UnableToParseElement(String),

    /// An unknown element type was encountered
    #[error("Unknown element")]
    UnknownElement,

    /// A repealed element was encountered
    ///
    /// Repealed elements lack a USLM ID and are difficult to parse in a way
    /// that makes sense with the current workflow. Since by definition they're
    /// not applicable to the law, they are currently not supported.
    #[error("Repealed element")]
    RepealedElement,

    /// A reserved element was encountered
    ///
    /// Reserved elements lack a USLM ID and are difficult to parse in a way
    /// that makes sense with the current workflow. Since by definition they're
    /// placeholders, they are currently not supported.
    #[error("Reserved element")]
    ReservedElement,
}

struct TextContents {
    pub heading: Option<String>,
    pub chapeau: Option<String>,
    pub proviso: Option<String>,
    pub content: Option<String>,
    pub continuation: Option<String>,
}

pub struct Number {
    pub value: String,
    pub display: String,
}

pub type Result<T> = std::result::Result<T, ParseError>;

fn check_attr(node: &roxmltree::Node, attr: &str, val: &str) -> bool {
    match node.attribute(attr) {
        None => false,
        Some(s) => s == val,
    }
}

/// Parse a USLM XML string into a USLMElement tree
///
/// This function parses XML content directly from a string, enabling unit testing
/// without filesystem access and in-memory parsing workflows.
///
/// The parser:
/// - Extracts the hierarchical structure of the document
/// - Generates both structural paths and USLM IDs for elements
/// - Parses all text content fields (heading, chapeau, proviso, content, continuation)
/// - Preserves source credits and metadata
///
/// # Arguments
///
/// * `xml_str` - The USLM XML content as a string
/// * `date` - Publication date in "YYYY-MM-DD" format (e.g., "2025-07-18")
///
/// # Returns
///
/// A `USLMElement` tree representing the entire document hierarchy, or a
/// `ParseError` if parsing fails.
///
/// # Supported Document Types
///
/// - **US Code** (`<uscDoc>`): USC titles and their appendices
/// - **Public Laws** (`<pLaw>`): Enacted legislation
///
/// # Examples
///
/// ```no_run
/// use words_to_data::uslm::parser::parse_from_str;
///
/// let xml = std::fs::read_to_string("usc07.xml").unwrap();
/// let element = parse_from_str(&xml, "2025-07-18").unwrap();
/// ```
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The XML is malformed
/// - The document type is not recognized
/// - Required elements are missing from the XML structure
pub fn parse_from_str(xml_str: &str, date: &str) -> Result<USLMElement> {
    let doc = roxmltree::Document::parse(xml_str)?;

    let top_level_node = doc
        .descendants()
        .find(|n| n.tag_name().name() == "uscDoc" || n.has_tag_name("pLaw"));

    let document_type = match top_level_node {
        None => {
            return Err(ParseError::UnsupportedDocumentType(
                "Can't resolve top-level document".to_string(),
            ));
        }
        Some(x) => {
            // For pLaw documents, extract bill_id from preface/docNumber or meta/docNumber
            if x.tag_name().name() == "pLaw" {
                // First try preface/docNumber (format: "119-21")
                let preface = x.children().find(|n| n.has_tag_name("preface"));
                if let Some(pref) = preface {
                    if let Some(doc_num) = pref.children().find(|n| n.has_tag_name("docNumber")) {
                        if let Some(text) = doc_num.text() {
                            DocumentType::Bill {
                                bill_type: BillType::PublicLaw,
                                bill_id: text.to_string(),
                            }
                        } else {
                            return Err(ParseError::UnsupportedDocumentType(
                                "pLaw missing docNumber text".to_string(),
                            ));
                        }
                    } else {
                        return Err(ParseError::UnsupportedDocumentType(
                            "pLaw missing docNumber".to_string(),
                        ));
                    }
                } else {
                    return Err(ParseError::UnsupportedDocumentType(
                        "pLaw missing preface tag".to_string(),
                    ));
                }
            } else {
                // For USC and other documents, use the original logic
                let meta_tag = x.children().find(|n| n.has_tag_name("meta"));
                let type_str: Option<&str> = match meta_tag {
                    Some(meta) => {
                        let dc_type = meta.children().find(|n| n.has_tag_name("type"));
                        dc_type.and_then(|n| n.text())
                    }
                    None => None,
                };
                DocumentType::from_str(x.tag_name().name(), type_str)?
            }
        }
    };
    // This is guaranteed safe by the matcher above
    let top_level_node = top_level_node.unwrap();

    // For USC documents, create a uscode container and parse title as direct child
    match &document_type {
        DocumentType::USCode { .. } => {
            let d = crate::date::date_str_to_date(date)?;

            // Create the container document type with USCType::USCode
            let container_doc_type = DocumentType::USCode {
                usc_type: USCType::USCode,
            };

            // Create the uscode container element
            let container_data = ElementData {
                path: "uscode".to_string(),
                element_type: ElementType::USCodeDocument,
                document_type: container_doc_type.clone(),
                date: d,
                number_value: String::new(),
                number_display: String::new(),
                verbose_name: "US Code".to_string(),
                heading: None,
                chapeau: None,
                proviso: None,
                content: None,
                continuation: None,
                uslm_id: None,
                uslm_uuid: None,
                source_credits: vec![],
            };

            // Find <main> and parse its children as direct children of the container
            let main_node = top_level_node
                .children()
                .find(|n| n.has_tag_name("main"))
                .unwrap_or(top_level_node);

            let mut children: Vec<USLMElement> = Vec::new();
            for child in main_node.children() {
                let child_element = parse_element(
                    child,
                    &container_doc_type,
                    date,
                    Some("US Code"),
                    Some("uscode".to_string()),
                    None,
                    1,
                );
                match child_element {
                    Ok(e) => children.push(e),
                    Err(ParseError::UnknownElement)
                    | Err(ParseError::RepealedElement)
                    | Err(ParseError::ReservedElement) => {}
                    Err(other) => return Err(other),
                }
            }

            Ok(USLMElement {
                data: container_data,
                children,
            })
        }
        // For other document types (bills), use the original logic
        _ => {
            let element = parse_element(top_level_node, &document_type, date, None, None, None, 0)?;
            Ok(element)
        }
    }
}

/// Parse a USLM XML document into a USLMElement tree
///
/// This is the main entry point for parsing USLM documents from files. It handles both
/// US Code titles and Public Laws (bills), automatically detecting the document
/// type from the XML structure.
///
/// For in-memory parsing without filesystem access, use `parse_from_str()` instead.
///
/// The parser:
/// - Extracts the hierarchical structure of the document
/// - Generates both structural paths and USLM IDs for elements
/// - Parses all text content fields (heading, chapeau, proviso, content, continuation)
/// - Preserves source credits and metadata
///
/// # Arguments
///
/// * `path` - Path to the USLM XML file to parse
/// * `date` - Publication date in "YYYY-MM-DD" format (e.g., "2025-07-18")
///
/// # Returns
///
/// A `USLMElement` tree representing the entire document hierarchy, or a
/// `ParseError` if parsing fails.
///
/// # Supported Document Types
///
/// - **US Code** (`<uscDoc>`): USC titles and their appendices
/// - **Public Laws** (`<pLaw>`): Enacted legislation
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::parser::parse;
///
/// // Parse a USC title - root is uscode container, title is first child
/// let usc = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
/// assert_eq!(usc.data.path, "uscode");
/// assert_eq!(usc.children[0].data.uslm_id.as_ref().unwrap(), "/us/usc/t7");
///
/// // Parse a public law
/// let bill = parse("tests/test_data/bills/119-hr-1/bill_119_hr_1.xml", "2025-07-04").unwrap();
/// assert_eq!(bill.data.uslm_id.unwrap(), "/us/pl/119-21");
/// ```
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The file cannot be read
/// - The XML is malformed
/// - The document type is not recognized
/// - Required elements are missing from the XML structure
pub fn parse(path: &str, date: &str) -> Result<USLMElement> {
    let xml_str = load_xml_file(path)?;
    parse_from_str(&xml_str, date)
}

fn rewrap_str(s: Option<&str>) -> Option<String> {
    s.map(String::from)
}

/// Normalize Unicode typographic quotes to their ASCII equivalents.
///
/// USLM and bill XML documents use Unicode curly quotes. Callers
/// expect plain ASCII quote characters, so we normalize them here
/// at the point of text extraction rather than ad-hoc at call sites.
pub(crate) fn normalize_quotes(s: &str) -> String {
    s.replace(['\u{2018}', '\u{2019}'], "'") // single curly quotes → apostrophe
        .replace(['\u{201C}', '\u{201D}'], "\"") // double curly quotes → quotation mark
}

/// Extract source credits from a USLM element
///
/// This function finds all `<sourceCredit>` child nodes and extracts their references,
/// splitting them into separate SourceCredit objects when separated by semicolons.
///
/// # Arguments
///
/// * `node` - The XML node to extract source credits from
///
/// # Returns
///
/// A vector of SourceCredit objects, potentially empty if no source credits exist
fn extract_source_credits(node: &roxmltree::Node) -> Vec<SourceCredit> {
    let source_credit_nodes = node.children().filter(|n| n.has_tag_name("sourceCredit"));

    let mut result = Vec::new();

    for sc_node in source_credit_nodes {
        let mut current_refs = Vec::new();

        // Walk through descendants to find refs and semicolons
        for descendant in sc_node.descendants() {
            if descendant.has_tag_name("ref") {
                if let Some(href) = descendant.attribute("href") {
                    let description = descendant.text().unwrap_or("").to_string();
                    current_refs.push(RefPair {
                        ref_id: href.to_string(),
                        description,
                    });
                }
            } else if let Some(text) = descendant.text()
                && text.contains(';')
            {
                // Finalize current group
                if !current_refs.is_empty() {
                    result.push(SourceCredit {
                        ref_pairs: current_refs.clone(),
                    });
                    current_refs.clear();
                }
            }
        }

        // Add final group
        if !current_refs.is_empty() {
            result.push(SourceCredit {
                ref_pairs: current_refs,
            });
        }
    }

    result
}

fn parse_element(
    node: roxmltree::Node,
    document_type: &DocumentType,
    date: &str,
    parent_name: Option<&str>,
    parent_structural_path: Option<String>,
    parent_uslm_path: Option<String>,
    _depth: usize,
) -> Result<USLMElement> {
    if check_attr(&node, "status", "repealed") {
        return Err(ParseError::RepealedElement);
    }
    if check_attr(&node, "status", "reserved") {
        return Err(ParseError::ReservedElement);
    }
    let element_type = ElementType::from_str(node.tag_name().name())
        .expect("When this expect was written, all match cases were Ok()");
    if matches!(element_type, ElementType::Unknown) {
        return Err(ParseError::UnknownElement);
    }
    let xml_identifier = rewrap_str(node.attribute("identifier"));
    let uslm_uuid = rewrap_str(node.attribute("id"));

    let number = extract_number(element_type, &node)?;
    // TODO
    // Source Credits
    let verbose_name = match parent_name {
        None => number.display.clone(),
        Some(s) => {
            format!("{} {}", s, number.display.clone())
        }
    };
    let text_contents = extract_text_contents(&node);

    // Generate structural path (includes all elements like Level)
    let structural_path = generate_structural_path(
        element_type,
        &number.value,
        parent_structural_path.as_deref(),
    );

    // Generate USLM path for USLM-significant elements only
    // Structural-only elements like Level will have None for uslm_id
    let uslm_id = if should_include_in_uslm_path(element_type) {
        match xml_identifier {
            Some(xml_id) => Some(xml_id),
            None => match element_type {
                ElementType::PublicLawDocument => Some(format!("/us/pl/{}", number.value)),
                _ => {
                    return Err(ParseError::UnableToParseElement(format!(
                        "XML identifier missing for element type {:?}",
                        element_type
                    )));
                }
            },
        }
    } else {
        // Structural-only elements don't have USLM identifiers
        None
    };

    let d = crate::date::date_str_to_date(date)?;

    // Extract source credits from the node
    let source_credits = extract_source_credits(&node);

    let element_data = ElementData {
        path: structural_path.clone(),
        uslm_id: uslm_id.clone(),
        uslm_uuid,
        document_type: document_type.clone(),
        element_type,
        date: d,
        number_value: number.value,
        number_display: number.display,
        verbose_name: verbose_name.clone(),
        heading: text_contents.heading,
        chapeau: text_contents.chapeau,
        proviso: text_contents.proviso,
        content: text_contents.content,
        continuation: text_contents.continuation,
        source_credits,
    };

    let cont_node = match matches!(element_type, uslm::ElementType::USCodeDocument) {
        // USCDoc headers look like this:
        // <uscDoc xmlns="http://xml.house.gov/schemas/uslm/1.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xsi:schemaLocation="http://xml.house.gov/schemas/uslm/1.0 USLM-1.0.15.xsd" xml:lang="en" identifier="/us/usc/t26">
        //   <meta>
        //     <dc:title>Title 26</dc:title>
        //     <dc:type>USCTitle</dc:type>
        // ...
        // </meta>
        //   <main>
        //     <title id="id2ff1c6b3-76ce-11f0-a3ab-d79a777afc56" identifier="/us/usc/t26">
        // ...
        // So we want to skip right to main and keep going from there
        // TODO perhaps make the USCodeDocument type have meta as an additional field
        true => {
            // Some USC data like titles have a <main> child node. We need to step into it if it exists
            let main_node = node.children().find(|n| n.has_tag_name("main"));
            match main_node {
                Some(x) => x,
                None => node,
            }
        }
        false => node,
    };
    // println!(
    //     "{}{}: {}",
    //     "-".repeat(depth),
    //     format!("{:?}", &element_type),
    //     verbose_name
    // );

    let mut children: Vec<USLMElement> = Vec::new();
    for child in cont_node.children() {
        // For USLM path, pass the generated USLM ID if this element has one,
        // otherwise pass through the parent's USLM path
        let child_parent_uslm_path = uslm_id.clone().or_else(|| parent_uslm_path.clone());

        let child_element = parse_element(
            child,
            document_type,
            date,
            Some(verbose_name.as_str()),
            Some(structural_path.clone()),
            child_parent_uslm_path,
            _depth + 1,
        );
        match child_element {
            Ok(e) => {
                //let box_elem = Box::new(e);
                children.push(e);
            }
            Err(err) => match err {
                // We skip these elements and consider them to be dead parts of the document tree
                ParseError::UnknownElement => {}
                ParseError::RepealedElement => {}
                ParseError::ReservedElement => {}
                // All other errors should cause the parser to stop and propogate the issue
                other => {
                    return Err(other);
                }
            },
        }
    }
    let element = USLMElement {
        data: element_data,
        children,
    };
    Ok(element)
}

pub fn extract_number(element_type: ElementType, node: &roxmltree::Node) -> Result<Number> {
    // Extract <number> tag data
    match node.children().find(|n| n.has_tag_name("num")) {
        None => {
            match element_type {
                // USCode Documents (top-level) don't have a <num> tag
                ElementType::USCodeDocument => {
                    // Unwrapping here because I want this to explode if it's ever true
                    let meta = node
                        .children()
                        .find(|n| n.has_tag_name("meta"))
                        .expect("meta tag should always be there");
                    let number = meta
                        .children()
                        .find(|n| n.has_tag_name("docNumber"))
                        .expect("should always be there");
                    Ok(Number {
                        value: extract_text(Some(number)).unwrap(),
                        display: String::new(),
                    })
                }
                // Public Law Documents also get their number from meta/docNumber
                ElementType::PublicLawDocument => {
                    let meta = node
                        .children()
                        .find(|n| n.has_tag_name("meta"))
                        .expect("pLaw should have meta tag");
                    let number = meta
                        .children()
                        .find(|n| n.has_tag_name("docNumber"))
                        .expect("should always be there");
                    let congress = meta
                        .children()
                        .find(|n| n.has_tag_name("congress"))
                        .expect("should always be there");
                    let num_val = format!(
                        "{}-{}",
                        extract_text(Some(congress)).unwrap(),
                        extract_text(Some(number)).unwrap()
                    );
                    Ok(Number {
                        value: num_val,
                        display: String::new(),
                    })
                }
                ElementType::Level => match node.attribute("id") {
                    None => Err(ParseError::UnableToParseElement(
                        "<Level> element has neither a <num> or <id> field".to_string(),
                    )),
                    Some(n) => Ok(Number {
                        value: String::from(n),
                        display: format!("Level {}", n),
                    }),
                },
                _ => Err(ParseError::UnableToParseElement(format!(
                    "'{:?}': missing <num> tag",
                    element_type
                ))),
            }
        }
        Some(n) => {
            let num_val = match n.attribute("value") {
                None => String::new(),
                Some(val) => String::from(val),
            };
            let display_val = extract_text(Some(n)).unwrap_or_default();
            Ok(Number {
                value: num_val,
                display: display_val,
            })
        }
    }
}

fn extract_text(node: Option<roxmltree::Node>) -> Option<String> {
    match node {
        None => None,
        Some(n) => n.text().map(normalize_quotes),
    }
}

fn extract_text_contents(node: &roxmltree::Node) -> TextContents {
    // TODO handle amending actions
    // TODO deal with page data funkiness
    TextContents {
        heading: extract_text(node.children().find(|n| n.has_tag_name("heading"))),
        chapeau: extract_text(node.children().find(|n| n.has_tag_name("chapeau"))),
        proviso: extract_text(node.children().find(|n| n.has_tag_name("proviso"))),
        content: extract_text(node.children().find(|n| n.has_tag_name("content"))),
        continuation: extract_text(node.children().find(|n| n.has_tag_name("continuation"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_quotes, parse};

    #[test]
    fn test_normalize_quotes_all_variants() {
        assert_eq!(normalize_quotes("\u{2018}hello\u{2019}"), "'hello'");
        assert_eq!(normalize_quotes("\u{201C}hello\u{201D}"), "\"hello\"");
    }

    #[test]
    fn test_normalize_quotes_mixed() {
        let input = "It\u{2019}s called \u{201C}agriculture\u{201D}.";
        assert_eq!(normalize_quotes(input), "It's called \"agriculture\".");
    }

    #[test]
    fn test_normalize_quotes_no_change() {
        let plain = "It's called \"agriculture\".";
        assert_eq!(normalize_quotes(plain), plain);
    }

    #[test]
    fn test_parse_normalizes_quotes_in_uslm() {
        let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
            .expect("test data should parse");
        let all_text = collect_text(&element);
        assert!(
            !all_text.contains('\u{2018}')
                && !all_text.contains('\u{2019}')
                && !all_text.contains('\u{201C}')
                && !all_text.contains('\u{201D}'),
            "parsed text should contain no Unicode typographic quotes"
        );
    }

    fn collect_text(elem: &crate::uslm::USLMElement) -> String {
        let mut buf = String::new();
        for s in [
            &elem.data.heading,
            &elem.data.chapeau,
            &elem.data.content,
            &elem.data.proviso,
            &elem.data.continuation,
        ]
        .into_iter()
        .flatten()
        {
            buf.push_str(s);
        }
        for child in &elem.children {
            buf.push_str(&collect_text(child));
        }
        buf
    }
}
