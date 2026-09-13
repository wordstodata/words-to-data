use std::{str::FromStr, sync::Arc};

use thiserror::Error;

use crate::{
    document::{DocumentNode, NodeData},
    io::load_xml_file,
    uslm::{
        self, BillType, DocumentType, ElementType, RefPair, SourceCredit, USCType, USLMError,
        UslmFacts,
        path::{path_segment_from_heading, should_include_in_uslm_path},
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

    /// Quoted statutory text encountered outside a `quotedContent` wrapper
    ///
    /// Amendment language quotes the text it enacts. That quoted text is not
    /// law in force, and the publisher almost always wraps it in
    /// `quotedContent`, which this parser does not descend into. Nine elements
    /// in title 26 carry no wrapper and are marked only by a quotation mark
    /// opening the number. Without this they enter the tree as provisions that
    /// the U.S. Code website does not render, and become searchable, diffable
    /// locations an annotation can name (#86).
    ///
    /// The subtree goes with the element. In title 26 that costs one real
    /// provision, 1563(f)(5)(B), which sits inside a quoted block because the
    /// publisher closed the paragraph it belongs to before opening the quote.
    /// Every identifier in that block is generated from position rather than
    /// asserted, and the position is wrong, so there is no sound home to move
    /// it to. Dropping it is the decision; #87 records what was lost and the
    /// signals a future rule could use to recover it.
    #[error("Quoted amendment text")]
    QuotedContent,
}

/// An element the parser dropped although it held law.
///
/// The parser skips an element whose name it does not know. For a leaf that is
/// right: a table of contents or a note is not law. For a container it is data
/// loss, because a container's children are law. Four US Code appendices held
/// two to four elements each for this reason, and nothing said so (#110).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedContainer {
    /// The XML tag name the parser does not know, such as `article`.
    pub element_name: String,
    /// The structural path of the element that held it.
    pub parent_path: String,
    /// How many structural children went with it.
    pub structural_children: usize,
}

impl std::fmt::Display for DroppedContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "warning: unknown USLM element <{}> in {} was dropped with {} structural children below it",
            self.element_name, self.parent_path, self.structural_children
        )
    }
}

/// What one parse dropped that a reader needs to know about.
///
/// A count alone would not help: the report names each container and where it
/// sat, so the reader can see which body of law is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseReport {
    /// Every unknown element that held law, in the order the parser met them.
    pub dropped_containers: Vec<DroppedContainer>,
}

impl ParseReport {
    /// True when the parse dropped nothing a reader needs to know about.
    pub fn is_empty(&self) -> bool {
        self.dropped_containers.is_empty()
    }

    /// Write the report to stderr, so it reaches the person who ran the command.
    ///
    /// The crate carries no logger, and the CLI writes its own warnings with
    /// `eprintln!`, so the parser does the same. Every entry point that hides
    /// the report calls this, which keeps one place to change.
    pub fn print_to_stderr(&self) {
        for dropped in &self.dropped_containers {
            eprintln!("{dropped}");
        }
    }
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

/// Parse a USLM XML string into a DocumentNode tree
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
/// A `DocumentNode` tree representing the entire document hierarchy, or a
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
pub fn parse_from_str(xml_str: &str, date: &str) -> Result<DocumentNode> {
    let (element, report) = parse_from_str_with_report(xml_str, date)?;
    report.print_to_stderr();
    Ok(element)
}

/// Parse a USLM XML string, and keep the report of what the parse dropped
///
/// Same parse as [`parse_from_str`], except that the caller receives the
/// [`ParseReport`] instead of having it printed to stderr. Use this when the
/// caller decides how a dropped container is shown.
///
/// # Examples
///
/// ```no_run
/// use words_to_data::uslm::parser::parse_from_str_with_report;
///
/// let xml = std::fs::read_to_string("usc28a.xml").unwrap();
/// let (element, report) = parse_from_str_with_report(&xml, "2025-07-18").unwrap();
/// assert!(report.is_empty() || !report.dropped_containers.is_empty());
/// ```
pub fn parse_from_str_with_report(
    xml_str: &str,
    date: &str,
) -> Result<(DocumentNode, ParseReport)> {
    let mut report = ParseReport::default();
    let element = parse_document(xml_str, date, &mut report)?;
    Ok((element, report))
}

fn parse_document(xml_str: &str, date: &str, report: &mut ParseReport) -> Result<DocumentNode> {
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
            let container_facts = UslmFacts {
                number_value: String::new(),
                number_display: String::new(),
                verbose_name: "US Code".to_string(),
                uslm_id: None,
                uslm_uuid: None,
                document_type: container_doc_type.clone(),
                source_credits: vec![],
            };
            let container_data = NodeData::new(
                "uscode",
                ElementType::USCodeDocument.node_type(&container_doc_type),
                d,
            )
            .with_payload(container_facts.to_payload()?);

            // Find <main> and parse its children as direct children of the container
            let main_node = top_level_node
                .children()
                .find(|n| n.has_tag_name("main"))
                .unwrap_or(top_level_node);

            let mut children: Vec<DocumentNode> = Vec::new();
            for child in main_node.children() {
                let child_element = parse_element(
                    child,
                    &container_doc_type,
                    date,
                    Some("US Code"),
                    Some("uscode"),
                    None,
                    1,
                    report,
                );
                match child_element {
                    Ok(e) => children.push(e),
                    Err(ParseError::UnknownElement)
                    | Err(ParseError::RepealedElement)
                    | Err(ParseError::ReservedElement)
                    | Err(ParseError::QuotedContent) => {}
                    Err(other) => return Err(other),
                }
            }

            Ok(DocumentNode {
                data: container_data,
                children,
            })
        }
        // For other document types (bills), use the original logic
        _ => {
            let element = parse_element(
                top_level_node,
                &document_type,
                date,
                None,
                None,
                None,
                0,
                report,
            )?;
            Ok(element)
        }
    }
}

/// Parse a USLM XML document into a DocumentNode tree
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
/// A `DocumentNode` tree representing the entire document hierarchy, or a
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
///
/// // Parse a public law
/// let bill = parse("tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml", "2025-07-04").unwrap();
/// ```
///
/// # Errors
///
/// Returns `ParseError` if:
/// - The file cannot be read
/// - The XML is malformed
/// - The document type is not recognized
/// - Required elements are missing from the XML structure
pub fn parse(path: &str, date: &str) -> Result<DocumentNode> {
    let xml_str = load_xml_file(path)?;
    parse_from_str(&xml_str, date)
}

/// Parse a USLM XML file, and keep the report of what the parse dropped
///
/// Same parse as [`parse`], except that the caller receives the [`ParseReport`]
/// instead of having it printed to stderr.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::parser::parse_with_report;
///
/// let (element, report) =
///     parse_with_report("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18").unwrap();
/// assert!(!element.children.is_empty());
/// // Title 9 holds no container the parser cannot name.
/// assert!(report.is_empty());
/// ```
pub fn parse_with_report(path: &str, date: &str) -> Result<(DocumentNode, ParseReport)> {
    let xml_str = load_xml_file(path)?;
    parse_from_str_with_report(&xml_str, date)
}

fn rewrap_str(s: Option<&str>) -> Option<Arc<str>> {
    s.map(Arc::from)
}

fn rewrap_string(s: Option<String>) -> Option<Arc<str>> {
    s.map(Arc::from)
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

/// True when a number opens with a quotation mark, marking quoted text.
///
/// The publisher writes the enacted text of an amendment inside quotation
/// marks. Where the surrounding `quotedContent` wrapper is missing, the opening
/// mark on the number is the only thing that distinguishes `“(2)` — text a bill
/// proposes — from `(2)`, the provision in force beside it.
fn is_quoted_number(display: &str) -> bool {
    matches!(
        display.trim_start().chars().next(),
        // Curly and straight, since the source uses both.
        Some('\u{201C}') | Some('"')
    )
}

/// How many children of this node are law the parser knows how to model.
///
/// A child counts only when the parser knows its name and it comes from the
/// same vocabulary as the node that holds it. `<meta>` holds a Dublin Core
/// `<dc:title>`, which shares a tag name with a US Code title but is metadata,
/// not law.
fn structural_child_count(node: &roxmltree::Node) -> usize {
    node.children()
        .filter(|child| {
            child.is_element()
                && child.tag_name().namespace() == node.tag_name().namespace()
                && matches!(
                    ElementType::from_str(child.tag_name().name()),
                    Ok(element_type) if element_type != ElementType::Unknown
                )
        })
        .count()
}

/// Record an unknown element that takes law with it when the parser drops it.
///
/// An unknown leaf is not recorded. Dropping a table of contents or a note is
/// what the parser is supposed to do, so a report of it would be noise that
/// hides the entries that matter.
fn record_if_container(
    node: &roxmltree::Node,
    parent_structural_path: Option<&str>,
    report: &mut ParseReport,
) {
    if !node.is_element() {
        return;
    }
    let structural_children = structural_child_count(node);
    if structural_children == 0 {
        return;
    }
    report.dropped_containers.push(DroppedContainer {
        element_name: node.tag_name().name().to_string(),
        parent_path: parent_structural_path.unwrap_or("").to_string(),
        structural_children,
    });
}

// The parent context this function needs is already long, and the report makes
// one argument more. Grouping them is a change worth making on its own.
#[allow(clippy::too_many_arguments)]
fn parse_element(
    node: roxmltree::Node,
    document_type: &DocumentType,
    date: &str,
    parent_name: Option<&str>,
    parent_structural_path: Option<&str>,
    parent_uslm_path: Option<&str>,
    _depth: usize,
    report: &mut ParseReport,
) -> Result<DocumentNode> {
    if check_attr(&node, "status", "repealed") {
        return Err(ParseError::RepealedElement);
    }
    if check_attr(&node, "status", "reserved") {
        return Err(ParseError::ReservedElement);
    }
    // The parser does not descend into quoted text: see `ParseError::QuotedContent`.
    // Naming the wrapper here keeps it out of the report, because dropping it is
    // a decision rather than a gap in what the parser knows.
    if node.has_tag_name("quotedContent") {
        return Err(ParseError::QuotedContent);
    }
    let element_type = ElementType::from_str(node.tag_name().name())
        .expect("When this expect was written, all match cases were Ok()");
    if matches!(element_type, ElementType::Unknown) {
        record_if_container(&node, parent_structural_path, report);
        return Err(ParseError::UnknownElement);
    }
    let xml_identifier = node.attribute("identifier");
    let uslm_uuid = rewrap_str(node.attribute("id"));

    let number = extract_number(element_type, &node)?;
    if is_quoted_number(&number.display) {
        return Err(ParseError::QuotedContent);
    }
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
    let structural_path =
        generate_structural_path(element_type, &number.value, parent_structural_path);

    // Generate USLM path for USLM-significant elements only
    // Structural-only elements like Level will have None for uslm_id
    let uslm_id = if should_include_in_uslm_path(element_type) {
        match xml_identifier {
            Some(xml_id) => Some(Arc::from(xml_id)),
            None => match element_type {
                ElementType::PublicLawDocument => {
                    Some(Arc::from(format!("/us/pl/{}", number.value).as_str()))
                }
                ElementType::Appendix => {
                    Some(Arc::from(format!("/us/usc/t{}", number.value).as_str()))
                }
                // The publisher supplies no identifier for a section it has
                // omitted or transferred, because the section is no longer law
                // in force and has no citation. A USLM ID belongs to the
                // publisher, and many documents supply none, so the element
                // keeps its structural path and carries no USLM ID. Until #110
                // this stopped the whole parse, which is how the compiled acts
                // in the title 5 appendix first came to light.
                _ => None,
            },
        }
    } else {
        // Structural-only elements don't have USLM identifiers
        None
    };

    let d = crate::date::date_str_to_date(date)?;

    // Extract source credits from the node
    let source_credits = extract_source_credits(&node);

    // The publisher's own naming of this element is a USLM fact, so it travels
    // in the payload rather than in a core field. The core keeps the path, the
    // type, the date and the text (#129).
    let facts = UslmFacts {
        number_value: number.value.clone(),
        number_display: number.display.clone(),
        verbose_name: verbose_name.clone(),
        uslm_id: uslm_id.as_deref().map(str::to_string),
        uslm_uuid: uslm_uuid.as_deref().map(str::to_string),
        document_type: document_type.clone(),
        source_credits,
    };

    let element_data = NodeData {
        path: structural_path.clone().into(),
        node_type: element_type.node_type(document_type),
        date: d,
        heading: rewrap_string(text_contents.heading),
        chapeau: rewrap_string(text_contents.chapeau),
        proviso: rewrap_string(text_contents.proviso),
        content: rewrap_string(text_contents.content),
        continuation: rewrap_string(text_contents.continuation),
        // No per-node provenance. Every node of a release point came from the
        // same publisher by the same method, so recording it on each would state
        // one fact a million times over and cost a heap allocation each. A node
        // carries its own provenance where it *differs* from its neighbours' —
        // an opinion whose text was read by OCR is the case that needs it (#53).
        provenance: None,
        payload: Some(facts.to_payload()?),
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

    let mut children: Vec<DocumentNode> = Vec::new();
    for child in cont_node.children() {
        // For USLM path, pass the generated USLM ID if this element has one,
        // otherwise pass through the parent's USLM path
        let child_parent_uslm_path = uslm_id.clone().or_else(|| rewrap_str(parent_uslm_path));

        let child_element = parse_element(
            child,
            document_type,
            date,
            Some(verbose_name.as_str()),
            Some(structural_path.as_str()),
            child_parent_uslm_path.as_deref(),
            _depth + 1,
            report,
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
                ParseError::QuotedContent => {}
                // All other errors should cause the parser to stop and propogate the issue
                other => {
                    return Err(other);
                }
            },
        }
    }
    let element = DocumentNode {
        data: element_data,
        children,
    };
    Ok(element)
}

/// True when an element inside a heading annotates the name, rather than being
/// part of it.
///
/// A heading can carry a footnote: the raised number that points at it, and the
/// footnote text itself. Both sit inside `<heading>`, so text collected from the
/// whole element reads as part of the container's name when it is not. One
/// heading in the title 28 appendix runs to 313 characters this way.
fn is_footnote(node: &roxmltree::Node) -> bool {
    node.has_tag_name("note") || check_attr(node, "class", "footnoteRef")
}

/// The text of a node's `<heading>`, with any footnote left out.
fn heading_text_without_footnotes(node: &roxmltree::Node) -> Option<String> {
    fn collect(node: &roxmltree::Node, into: &mut String) {
        for child in node.children() {
            if child.is_text() {
                into.push_str(child.text().unwrap_or_default());
            } else if child.is_element() && !is_footnote(&child) {
                collect(&child, into);
            }
        }
    }

    let heading = node.children().find(|n| n.has_tag_name("heading"))?;
    let mut text = String::new();
    collect(&heading, &mut text);
    Some(text)
}

/// The number value for a container that carries no `<num>`.
///
/// A container that groups a body of law often has no number of its own: the
/// Federal Rules sit in `courtRules`, and an unnumbered `level` gathers sections
/// under a heading. Three sources are tried, best first (#115):
///
/// 1. The publisher's `identifier`, reduced to its last segment, so
///    `/us/usc/t28a/courtRules/Civil` gives `Civil`.
/// 2. The publisher's `<heading>`, reduced to a path segment, so `FEDERAL RULES
///    OF BANKRUPTCY PROCEDURE` gives `federal-rules-of-bankruptcy-procedure`.
/// 3. The XML `id`, which is a uuid no person can read or type. No container in
///    the committed release points reaches this.
///
/// Numbering by position was rejected. It reads like a number the publisher
/// gave, and every path below the container moves when the publisher inserts a
/// sibling above it.
fn numberless_container_value(node: &roxmltree::Node) -> Option<String> {
    let from_identifier = node
        .attribute("identifier")
        .and_then(|identifier| identifier.rsplit('/').next())
        .filter(|segment| !segment.is_empty())
        .map(String::from);

    from_identifier
        .or_else(|| {
            heading_text_without_footnotes(node)
                .as_deref()
                .and_then(path_segment_from_heading)
        })
        .or_else(|| node.attribute("id").map(String::from))
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
                ElementType::Level => match numberless_container_value(node) {
                    None => Err(ParseError::UnableToParseElement(
                        "<Level> element has no <num>, identifier, heading or id".to_string(),
                    )),
                    Some(value) => Ok(Number {
                        display: format!("Level {}", value),
                        value,
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
    node.map(|n| {
        let text = collect_all_text(&n);
        if text.is_empty() {
            return None;
        }
        Some(normalize_quotes(&text))
    })?
}

fn collect_all_text(node: &roxmltree::Node) -> String {
    let mut result = String::new();
    for child in node.children() {
        if child.is_text() {
            if let Some(t) = child.text() {
                result.push_str(t);
            }
        } else if child.is_element() {
            result.push_str(&collect_all_text(&child));
        }
    }
    result
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

    fn collect_text(elem: &crate::document::DocumentNode) -> String {
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
