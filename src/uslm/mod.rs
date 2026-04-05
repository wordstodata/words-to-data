use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::Date;

pub mod bill_parser;
pub mod parser;
pub mod path;

/// Errors that can occur when parsing or processing USLM documents
#[derive(Error, Debug)]
pub enum USLMError {
    /// An unknown or unsupported document type was encountered
    #[error("Unknown Document Type {0}")]
    UnknownDocumentType(String),

    /// An unknown or unsupported amending action was encountered
    #[error("Unknown Amending Action {0}")]
    UnknownAmendingAction(String),
}

/// The type of legislative document being parsed
///
/// USLM documents can be either US Code titles or Bills (such as Public Laws).
/// Each type has associated metadata that provides additional context.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    /// United States Code document (e.g., Title 7, Title 26)
    #[serde(rename = "us_code")]
    USCode {
        /// The specific type of USC document (Title or TitleAppendix)
        usc_type: USCType,
    },

    /// Bill document (e.g., Public Law)
    Bill {
        /// The type of bill (currently only PublicLaw is supported)
        bill_type: BillType,
        /// The bill identifier (e.g., "119-21" for the 119th Congress, 21st law)
        bill_id: String,
    },
}

impl DocumentType {
    /// Parse a document type from string representation with optional metadata
    ///
    /// # Arguments
    ///
    /// * `s` - The document type string (case-insensitive). Accepted values:
    ///   - For USC: "uscode", "us_code", "uscdoc"
    ///   - For Bills: "publiclaw", "public_law", "plaw"
    /// * `meta_str` - Additional metadata required for type disambiguation:
    ///   - For USC: "usctitle" or "usctitleappendix"
    ///   - For Bills: the bill ID (e.g., "119-21")
    ///
    /// # Returns
    ///
    /// Returns `Ok(DocumentType)` if parsing succeeds, or `Err(USLMError)` if:
    /// - The document type string is not recognized
    /// - Required metadata is missing
    /// - The metadata value is invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use words_to_data::uslm::{DocumentType, USCType, BillType};
    ///
    /// // Parse a USC Title
    /// let usc = DocumentType::from_str("uscode", Some("usctitle")).unwrap();
    /// match usc {
    ///     DocumentType::USCode { usc_type } => assert_eq!(usc_type, USCType::Title),
    ///     _ => panic!("Expected USCode variant"),
    /// }
    ///
    /// // Parse a Public Law
    /// let bill = DocumentType::from_str("publiclaw", Some("119-21")).unwrap();
    /// match bill {
    ///     DocumentType::Bill { bill_type, bill_id } => {
    ///         assert_eq!(bill_type, BillType::PublicLaw);
    ///         assert_eq!(bill_id, "119-21");
    ///     },
    ///     _ => panic!("Expected Bill variant"),
    /// }
    /// ```
    pub fn from_str(s: &str, meta_str: Option<&str>) -> Result<Self, USLMError> {
        match s.to_lowercase().as_str() {
            "publiclaw" | "public_law" | "plaw" => match meta_str {
                Some(val) => Ok(Self::Bill {
                    bill_type: BillType::PublicLaw,
                    bill_id: val.to_string(),
                }),
                None => Err(USLMError::UnknownDocumentType(
                    "Bill types must pass the bill_id as the meta_str parameter".to_string(),
                )),
            },
            "uscode" | "us_code" | "uscdoc" => match meta_str {
                Some(val) => match val.to_lowercase().as_str() {
                    "usctitle" => Ok(DocumentType::USCode {
                        usc_type: USCType::Title,
                    }),
                    "usctitleappendix" => Ok(DocumentType::USCode {
                        usc_type: USCType::TitleAppendix,
                    }),
                    _ => Err(USLMError::UnknownDocumentType(format!(
                        "Unhandled type for USCode document: {}",
                        val.to_lowercase()
                    ))),
                },
                None => Err(USLMError::UnknownDocumentType(
                    "USCode types need to provide a type_str".to_string(),
                )),
            },
            _ => Err(USLMError::UnknownDocumentType(s.to_string())),
        }
    }
}

/// The type of bill document
///
/// Currently only Public Laws are supported, but this enum allows for
/// future expansion to support other bill types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillType {
    /// A Public Law (enacted legislation)
    PublicLaw,
}

/// The type of United States Code document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum USCType {
    /// A standard USC Title
    Title,
    /// An appendix to a USC Title
    TitleAppendix,
}

/// The hierarchical type of an element within a legislative document
///
/// Legislative documents follow a strict hierarchy with various levels of organization.
/// This enum represents all possible element types that can appear in USLM documents.
///
/// # Hierarchy Examples
///
/// For US Code:
/// - Title > Subtitle > Chapter > Subchapter > Part > Section > Subsection > Paragraph
///
/// For Bills:
/// - Division > Title > Subtitle > Chapter > Section > Subsection > Paragraph
///
/// The `Level` type is a special structural element used when the hierarchy
/// doesn't follow the standard pattern. `Unknown` is used for unrecognized elements.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    /// The root element of a US Code document
    #[serde(rename = "us_code_document")]
    USCodeDocument,
    /// The root element of a Public Law document
    PublicLawDocument,
    /// A Title (top level division in USC, or subdivision in bills)
    Title,
    /// An Appendix to a title or section
    Appendix,
    /// A Subtitle (subdivision of a title)
    Subtitle,
    /// A Chapter (major subdivision)
    Chapter,
    /// A Subchapter (subdivision of a chapter)
    Subchapter,
    /// A Part (subdivision, often of a subchapter)
    Part,
    /// A Subpart (subdivision of a part)
    Subpart,
    /// A Section (the primary unit of law, e.g., "Section 174")
    Section,
    /// A Subsection (subdivision of a section, often lettered: a, b, c)
    Subsection,
    /// A Paragraph (subdivision of a subsection, often numbered: 1, 2, 3)
    Paragraph,
    /// A Subparagraph (subdivision of a paragraph, often lettered: A, B, C)
    Subparagraph,
    /// A Clause (subdivision of a subparagraph, often numbered: i, ii, iii)
    Clause,
    /// A Subclause (subdivision of a clause)
    Subclause,
    /// A Level element (generic structural container when hierarchy is non-standard)
    Level,
    /// An Item in an enumerated list
    Item,
    /// A Subitem (subdivision of an item)
    Subitem,
    /// A Subsubitem (subdivision of a subitem)
    Subsubitem,
    /// A Division (top-level subdivision in some bills)
    Division,
    /// A Subdivision
    Subdivision,
    /// An unknown or unrecognized element type
    Unknown,
}

impl std::str::FromStr for ElementType {
    type Err = USLMError;

    /// Parse an element type from its string representation
    ///
    /// This implementation is case-insensitive and accepts various common names
    /// for element types. Unknown strings are mapped to `ElementType::Unknown`
    /// rather than returning an error.
    fn from_str(s: &str) -> Result<ElementType, USLMError> {
        match s.to_lowercase().as_str() {
            "title" => Ok(Self::Title),
            "subtitle" => Ok(Self::Subtitle),
            "chapter" => Ok(Self::Chapter),
            "subchapter" => Ok(Self::Subchapter),
            "part" => Ok(Self::Part),
            "subpart" => Ok(Self::Subpart),
            "section" => Ok(Self::Section),
            "subsection" => Ok(Self::Subsection),
            "paragraph" => Ok(Self::Paragraph),
            "subparagraph" => Ok(Self::Subparagraph),
            "clause" => Ok(Self::Clause),
            "subclause" => Ok(Self::Subclause),
            "level" => Ok(Self::Level),
            "item" => Ok(Self::Item),
            "subitem" => Ok(Self::Subitem),
            "subsubitem" => Ok(Self::Subsubitem),
            "division" => Ok(Self::Division),
            "subdivision" => Ok(Self::Subdivision),
            "publiclaw" | "public_law" | "plaw" => Ok(Self::PublicLawDocument),
            "uscode" | "us_code" | "uscdoc" => Ok(Self::USCodeDocument),
            "appendix" => Ok(Self::Appendix),
            _ => Ok(Self::Unknown),
        }
    }
}

/// The different text content fields that can be present in a legislative element
///
/// Legislative elements can have up to five distinct text fields, each serving
/// a specific purpose in the document structure. These fields are tracked
/// separately to enable precise change detection when comparing document versions.
/// One of five text fields that can appear in an element:
///
/// - Heading: Opening text that appears before enumerated sub-elements
/// - Chapeau: A conditional or qualifying clause (often starting with "Provided that")
/// - Proviso: The main text content of the element
/// - Content: Text that appears after all child elements
/// - Continuation: Text that appears after all child elements
///
/// **IMPORTANT**: Becuase continuations appear _after_ child elements, the full text of some elements require child elements to be present. This makes sense, to load a full section, you need the subsections, which need paragraphs which may need clauses, etc.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextContentField {
    /// The heading or title of the element (e.g., "Agricultural Programs")
    Heading,
    /// Opening text that appears before enumerated sub-elements
    Chapeau,
    /// A conditional or qualifying clause (often starting with "Provided that")
    Proviso,
    /// The main text content of the element
    Content,
    /// Text that appears after all child elements
    Continuation,
}

/// Types of amendments that can be made to existing law via a bill
///
/// When a bill modifies existing United States Code, it uses specific
/// amending actions to describe the type of change being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendingAction {
    /// Modify existing text
    Amend,
    /// Add new text or sections
    Add,
    /// Remove existing text or sections
    Delete,
    /// Insert new text at a specific location
    Insert,
    /// Change the designation or numbering of sections
    Redesignate,
    /// Remove an entire section or provision from the law
    Repeal,
    /// Relocate an element (may include redesignation)
    Move,
    /// Remove specific text within an element (finer than Delete)
    Strike,
    /// Remove specific text and replace with new text
    StrikeAndInsert,
}

impl FromStr for AmendingAction {
    type Err = USLMError;

    /// Parse an amending action from its string representation
    ///
    /// This implementation is case-insensitive. Returns an error if the
    /// action type is not recognized.
    fn from_str(s: &str) -> std::result::Result<Self, <Self as std::str::FromStr>::Err> {
        match s.to_lowercase().as_str() {
            "amend" => Ok(AmendingAction::Amend),
            "add" => Ok(AmendingAction::Add),
            "delete" => Ok(AmendingAction::Delete),
            "insert" => Ok(AmendingAction::Insert),
            "redesignate" => Ok(AmendingAction::Redesignate),
            "repeal" => Ok(AmendingAction::Repeal),
            "move" => Ok(AmendingAction::Move),
            "strike" => Ok(AmendingAction::Strike),
            "strikeandinsert" | "strike_and_insert" => Ok(AmendingAction::StrikeAndInsert),
            _ => Err(USLMError::UnknownAmendingAction(s.to_lowercase())),
        }
    }
}

impl AmendingAction {
    /// Extract all text from a node and its descendants
    #[allow(dead_code)]
    fn extract_all_text(node: &roxmltree::Node) -> String {
        let mut text = String::new();
        for descendant in node.descendants() {
            if let Some(t) = descendant.text() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(t);
            }
        }
        text
    }
}

/// A reference to a USC section found in a bill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct UscReference {
    /// The USLM path being referenced (e.g., "/us/usc/t7/s2025/c/1/A/ii")
    pub path: String,
    /// The human-readable text of the reference (e.g., "7 U.S.C. 2025(c)(1)(A)(ii)")
    pub display_text: String,
}

/// An amending action found in a bill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BillAmendment {
    /// Content-based ID: sha256("{bill_id}:{amending_text}")
    /// This provides a stable, deterministic identifier that works regardless of source format.
    pub id: String,

    /// Type of action (amend, add, delete, insert, redesignate, repeal)
    pub action_types: Vec<AmendingAction>,

    /// The text of the change
    pub amending_text: String,

    /// List of word-level changes that an amendment enacts
    pub changes: Vec<BillDiff>,
}

/// Actions caused by a bill amendment
///
/// This is designed to exist as single entries for every logical
/// amending action. For example, given the following amending text:
/// ```
///(B)
/// in subsection (b)--
///
///   (i)
///   by striking "specified research" and inserting "foreign research",
///
///
///   (ii)
///   by inserting "and which are attributable to foreign research (within the meaning of section 41(d)(4)(F))" before the period at the end, and
/// ```
/// we would annotate that with two Bill Diffs:
/// ```
/// {
///  "removed": ["specified"],
///  "added": ["foreign"]
/// }
/// ```
/// and
/// ```
/// {
///  "removed": [],
///  "added": [
///    "which",
///    "attributable",
///    "foreign",
///    "research",
///    "(within",
///    "meaning",
///    "section",
///    "41(d)(4)(F))"
///  ]
///}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BillDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// Source Credit Attribution
///
/// The Source credit can contain multiple `<ref>` elements, and they are separated logically
/// as new sources by a `;` between them in the XML. So when you encounter a `<SourceCredit>` element,
/// you should split the element into multiple `<SourceCredit>` elements, each with a single `<ref>` element.
///
/// **IMPORTANT**: Source credits point to USLM ID shaped paths, for example:
/// ```xml
/// <sourceCredit id="id2ffb3c99-76ce-11f0-a3ab-d79a777afc56">(<ref href="/us/act/1954-08-16/ch736">Aug. 16, 1954, ch. 736</ref>, <ref href="/us/stat/68A/3">68A Stat. 3</ref>; <ref href="/us/pl/99/514/s2">Pub. L. 99–514, § 2</ref>, <date date="1986-10-22">Oct. 22, 1986</date>, <ref href="/us/stat/100/2095">100 Stat. 2095</ref>.)</sourceCredit>
/// ```
/// They do not actually state change information, and the source credits are not guaranteed to cover all the bills that provided changes to the document. They are better thought of as an incomplete list of pointers. While useful, it is easy to confuse these with the full, definitive listing of bills that created the Element.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceCredit {
    /// The `<ref>` elements of the source credit
    pub ref_pairs: Vec<RefPair>,
}

/// A reference pair within a source credit
///
/// Contains the identifier and description for a single reference within
/// a source credit attribution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RefPair {
    /// The ID of the `<ref>` source credit
    pub ref_id: String,
    /// The description of the source credit
    pub description: String,
}

/// Metadata and content for a single element in a USLM document
///
/// This struct contains all the information about a legislative element,
/// including its position in the document hierarchy, identification paths,
/// display information, and text content.
///
/// # Path Systems
///
/// Each element has two types of paths:
///
/// 1. **Structural Path** (`path`): Includes all hierarchy elements, even
///    non-USLM ones like `Level`. Example:
///    `uscodedocument_26/title_26/subtitle_k/chapter_100/section_9834/level_1`
///
/// 2. **USLM ID** (`uslm_id`): Official USLM identifier following standard format.
///    Only present for elements in the USLM scheme. Example: `/us/usc/t26/s9834/a/1`
///
/// Combining the structural path with the date provides a unique identifier for
/// any element across all versions of the document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ElementData {
    /// The full structural path in the document for the element
    ///
    /// This includes all structural elements like Level that may not be part of the USLM identifier.
    /// Note that combining this with the date field gives a unique identifier for the document
    /// For example:
    ///
    /// uscodedocument_26/title_26/subtitle_k/chapter_100/subchapter_c/section_9834/level_1
    ///
    pub path: String,

    /// The type of this element in the legislative hierarchy
    pub element_type: ElementType,

    /// The type of document this element belongs to
    pub document_type: DocumentType,

    /// The date this version of the document was published
    pub date: Date,

    // Display
    /// The raw number or identifier value (e.g., "174", "a", "1")
    pub number_value: String,

    /// The formatted display version of the number (may include prefixes/suffixes)
    pub number_display: String,

    /// A human-readable name for this element (e.g., "Section 174")
    pub verbose_name: String,

    // Content Fields
    // These are the fields that we need to diff upon
    /// The heading or title text of the element
    pub heading: Option<String>,

    /// The words at the start of the element that appear before any enumerated items
    pub chapeau: Option<String>,

    /// A clause imposing a qualification, condition, or restriction
    pub proviso: Option<String>,

    /// The main text content of the element
    pub content: Option<String>,

    /// Text content that appears after all child elements
    pub continuation: Option<String>,

    // Metadata
    /// The USLM-standard identifier path for this element
    ///
    /// This follows the official USLM path format and excludes structural-only elements.
    /// For example: `/us/usc/t26/s1/a/1` or `/us/pl/119-21/s1/a`
    ///
    /// This is computed according to USLM standards for elements that are part of the
    /// USLM identifier scheme. If the XML provides an `identifier` attribute, it is
    /// validated to match this generated path.
    ///
    /// Structural-only elements like Level will have None here, as they are not part
    /// of the USLM identifier scheme.
    pub uslm_id: Option<String>,

    /// The USLM `id` attribute for an element
    ///
    /// Takes the form of a UUID, not guaranteed to exist
    pub uslm_uuid: Option<String>,

    /// Source credits and references for this element
    pub source_credits: Vec<SourceCredit>,
    //pub page_data: Option<PageData>, // TODO implement
}

impl ElementData {
    /// Retrieve the text content for a specific field
    ///
    /// # Arguments
    ///
    /// * `field` - The text content field to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some(String)` if the field has content, or `None` if the field
    /// is empty for this element.
    pub fn get_text_content(&self, field: TextContentField) -> Option<String> {
        match field {
            TextContentField::Heading => self.heading.clone(),
            TextContentField::Chapeau => self.chapeau.clone(),
            TextContentField::Proviso => self.proviso.clone(),
            TextContentField::Content => self.content.clone(),
            TextContentField::Continuation => self.continuation.clone(),
        }
    }
}

/// A hierarchical element in a USLM document tree
///
/// This struct represents a single element in a legislative document along with
/// all of its child elements, forming a tree structure that mirrors the document's
/// hierarchical organization.
///
/// # Structure
///
/// - `data`: Contains all metadata and text content for this element
/// - `children`: All direct child elements in document order
///
/// # Examples
///
/// A typical USC section might have a structure like:
///
/// ```text
/// Section 174 (USLMElement)
///   ├─ data: ElementData { element_type: Section, heading: "Research expenditures", ... }
///   └─ children:
///       ├─ Subsection (a) (USLMElement)
///       │   └─ children: [Paragraph (1), Paragraph (2), ...]
///       └─ Subsection (b) (USLMElement)
///           └─ children: [...]
/// ```
///
/// # Tree Navigation
///
/// Use the `find()` method to locate specific elements within the tree by their
/// structural path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct USLMElement {
    /// The metadata and content for this element
    pub data: ElementData,

    /// Child elements in document order
    pub children: Vec<USLMElement>,
}

impl USLMElement {
    /// Search for an element by its structural path
    ///
    /// Recursively searches this element and all descendants for an element
    /// with the specified path. The path must be a fully qualified structural
    /// path (e.g., "uscodedocument_7/title_7/chapter_1/section_1").
    ///
    /// # Arguments
    ///
    /// * `path` - The full structural path of the element to find
    ///
    /// # Returns
    ///
    /// Returns `Some(&USLMElement)` if an element with the matching path is found,
    /// or `None` if no such element exists in this tree.
    ///
    /// # Examples
    ///
    /// ```
    /// # use words_to_data::uslm::parser::parse;
    /// # let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    /// // Find a specific section
    /// let section = element.find("uscodedocument_7/title_7/chapter_1/section_2");
    /// assert!(section.is_some());
    ///
    /// // Non-existent path returns None
    /// let missing = element.find("uscodedocument_7/title_99");
    /// assert!(missing.is_none());
    /// ```
    pub fn find(&self, path: &str) -> Option<&USLMElement> {
        if path == self.data.path.as_str() {
            return Some(self);
        }
        let remaining_path = path.strip_prefix(self.data.path.as_str())?;
        let next_step: Vec<&str> = remaining_path.split("/").collect();
        assert!(next_step.len() > 1);

        let child_id = next_step[1];
        let child_vec: Vec<&USLMElement> = self
            .children
            .iter()
            .filter(|c| c.data.path.ends_with(child_id))
            .collect();
        if child_vec.is_empty() {
            None
        } else {
            assert!(child_vec.len() == 1);
            child_vec[0].find(path)
        }
    }
}
