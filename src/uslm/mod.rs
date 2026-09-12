//! USLM: one publisher's schema for US legislative material.
//!
//! Everything in this module belongs to that class of document. The core's tree
//! lives in [`crate::document`] and knows none of it. What the parser learns here
//! that only a USLM reader understands — an element's number, its USLM
//! identifier, its source credits — travels beside the node in a
//! [`ClassPayload`], as [`UslmFacts`]
//! (`docs/adr/0006-a-document-node-is-class-neutral.md`).
//!
//! One parser, two document classes. The same XML vocabulary describes the US
//! Code and a bill, so the words below the root are the same, and the namespace
//! of a node's type says which class it came from: `uscode` or `bill`.
//!
//! [`ElementType`] stays a closed enum on purpose. It is the parser's own
//! vocabulary, not a stored one: it is the list of XML tag names this parser
//! knows, and a tag it does not know is dropped rather than stored. What is
//! stored is a [`NodeType`], an open namespaced string, so a document class this
//! build has never heard of still round-trips.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::document::{ClassPayload, NodeData, NodeType};

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

    /// The node-type namespace for documents of this type.
    ///
    /// This is what replaced the old `document_type` field on a node. A class
    /// declaration
    /// belongs in one place, and the node type already carries one
    /// (`docs/adr/0006-a-document-node-is-class-neutral.md`).
    pub fn namespace(&self) -> &'static str {
        match self {
            Self::USCode { .. } => NodeType::USCODE,
            Self::Bill { .. } => NodeType::BILL,
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
    /// The entire US Code (container for all titles at a point in time)
    #[serde(rename = "us_code")]
    USCode,
    /// A standard USC Title
    /// TODO remove this in favor of USCode
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
    /// A Level element (generic structural container when hierarchy is
    /// non-standard). An appendix also uses one to group a body of law, such as
    /// the Federal Rules, that carries no number of its own.
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

impl ElementType {
    /// The word this element writes into a structural path: `section` in
    /// `section_174`.
    ///
    /// **These spellings are frozen.** They are in every path in every dataset,
    /// so changing one renames a provision and everything below it. They are
    /// spelt out here rather than derived from `Debug` for that reason: a stored
    /// name must not move when a Rust variant is renamed.
    ///
    /// This is also where [`ElementType::type_name`] gets almost every node type
    /// from, so a path and a type agree about what an element is called wherever
    /// there is no reason for them to differ.
    pub fn path_segment_name(self) -> &'static str {
        match self {
            Self::USCodeDocument => "uscodedocument",
            Self::PublicLawDocument => "publiclawdocument",
            Self::Title => "title",
            Self::Appendix => "appendix",
            Self::Subtitle => "subtitle",
            Self::Chapter => "chapter",
            Self::Subchapter => "subchapter",
            Self::Part => "part",
            Self::Subpart => "subpart",
            Self::Section => "section",
            Self::Subsection => "subsection",
            Self::Paragraph => "paragraph",
            Self::Subparagraph => "subparagraph",
            Self::Clause => "clause",
            Self::Subclause => "subclause",
            Self::Level => "level",
            Self::Item => "item",
            Self::Subitem => "subitem",
            Self::Subsubitem => "subsubitem",
            Self::Division => "division",
            Self::Subdivision => "subdivision",
            Self::Unknown => "unknown",
        }
    }

    /// The local half of this element's stored node type: `section` in
    /// `uscode.section`.
    ///
    /// Almost always the same word as [`ElementType::path_segment_name`], and
    /// taken from it, because a path and a type disagreeing about what an element
    /// is called would be a trap.
    ///
    /// **The two document roots are the exception, and are named here in full.**
    /// A path segment is frozen; a type is read by a person and by another
    /// party's reader, and these two are the only ones a path never shows:
    ///
    /// - `uscode.document` is the root of a US Code file. `uscode.title` was
    ///   considered and is wrong twice over — `ElementType::Title` already holds
    ///   that word, and this node is not a title. It is the file's root, with a
    ///   title *or* an appendix beneath it. The namespace already says `uscode`,
    ///   so `document` is all that is left to say.
    /// - `bill.public_law` is the root of a bill that has been enacted. The class
    ///   is the bill; being law is the state it reached. Nothing else under a bill
    ///   says whether it is enacted, and a reader plainly needs that in order to
    ///   report the document, so it belongs in the type rather than in
    ///   [`BillType`] inside the payload
    ///   (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    ///
    /// The two roots are therefore named unlike each other on purpose:
    /// title-versus-appendix is already visible for the US Code, in the path and
    /// in the child's own type, and enacted-versus-draft is visible nowhere else
    /// for a bill.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::USCodeDocument => "document",
            Self::PublicLawDocument => "public_law",
            other => other.path_segment_name(),
        }
    }

    /// The stored node type for this element, in the class's namespace.
    ///
    /// The namespace says which class of document the element came from, which
    /// is what `DocumentType` used to say in a field of its own. A section of the
    /// US Code is `uscode.section`; a section of a bill is `bill.section`. The
    /// same USLM word, and nothing stored says a bill is part of the US Code.
    ///
    /// # Examples
    ///
    /// ```
    /// use words_to_data::uslm::{BillType, DocumentType, ElementType, USCType};
    ///
    /// let usc = DocumentType::USCode { usc_type: USCType::Title };
    /// assert_eq!(
    ///     ElementType::Section.node_type(&usc).as_str(),
    ///     "uscode.section"
    /// );
    ///
    /// // The same word, in the namespace of the class it came from.
    /// let enacted = DocumentType::Bill {
    ///     bill_type: BillType::PublicLaw,
    ///     bill_id: "119-21".to_string(),
    /// };
    /// assert_eq!(
    ///     ElementType::Section.node_type(&enacted).as_str(),
    ///     "bill.section"
    /// );
    ///
    /// // The two document roots are named for a reader, not for a path.
    /// assert_eq!(
    ///     ElementType::USCodeDocument.node_type(&usc).as_str(),
    ///     "uscode.document"
    /// );
    /// assert_eq!(
    ///     ElementType::PublicLawDocument.node_type(&enacted).as_str(),
    ///     "bill.public_law"
    /// );
    /// ```
    pub fn node_type(self, document_type: &DocumentType) -> NodeType {
        NodeType::new(format!(
            "{}.{}",
            document_type.namespace(),
            self.type_name()
        ))
    }
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
            // The appendices group a whole body of law in a container that
            // carries no number of its own, so each one reads as a level: the
            // Federal Rules sit in `courtRules`, one rule in `courtRule`, an act
            // reprinted in an appendix in `compiledAct`, and the reorganization
            // plans in `reorganizationPlans` and `reorganizationPlan`. An
            // `article` groups the rules of the Federal Rules of Evidence, which
            // were absent from every dataset while the name was unknown (#122).
            // They group law; they are not a new unit of law (#110).
            "courtrules"
            | "courtrule"
            | "compiledact"
            | "reorganizationplans"
            | "reorganizationplan"
            | "article" => Ok(Self::Level),
            "publiclaw" | "public_law" | "plaw" => Ok(Self::PublicLawDocument),
            "uscode" | "us_code" | "uscdoc" => Ok(Self::USCodeDocument),
            "appendix" => Ok(Self::Appendix),
            _ => Ok(Self::Unknown),
        }
    }
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

/// What a USLM reader knows about a node that the core does not.
///
/// These are the facts that used to sit in core fields, where a court opinion had
/// to supply them or claim a `number_value` and a `document_type` it does not
/// have (#129). They now travel beside the node in a [`ClassPayload`], which the
/// core stores, hands back unchanged, and never reads.
///
/// Nothing here is needed in order to *report* a node. A reader that cannot open
/// this payload still has the node's path, its type, its date, its text and its
/// provenance, which is everything it takes to search, diff and quote a
/// provision. What it loses is the publisher's own naming of it.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::{UslmFacts, parser::parse};
///
/// let title = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18").unwrap();
/// let section = title.find("uscode/title_9/chapter_1/section_2").unwrap();
///
/// let facts = UslmFacts::of(&section.data).expect("a parsed USC section carries USLM facts");
/// assert_eq!(facts.number_value, "2");
/// assert_eq!(facts.uslm_id.as_deref(), Some("/us/usc/t9/s2"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UslmFacts {
    /// The raw number or identifier value (e.g., "174", "a", "1")
    pub number_value: String,

    /// The formatted display version of the number (may include prefixes/suffixes)
    pub number_display: String,

    /// A human-readable name for this element (e.g., "Section 174")
    pub verbose_name: String,

    /// The USLM-standard identifier path for this element
    ///
    /// This follows the official USLM path format and excludes structural-only
    /// elements. For example: `/us/usc/t26/s1/a/1` or `/us/pl/119-21/s1/a`
    ///
    /// Structural-only elements like `Level` carry `None`, as they are not part
    /// of the USLM identifier scheme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uslm_id: Option<String>,

    /// The USLM `id` attribute for an element
    ///
    /// Takes the form of a UUID, not guaranteed to exist
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uslm_uuid: Option<String>,

    /// Which kind of USLM document this element came from
    ///
    /// The node type's namespace already says US Code or public law. This keeps
    /// the rest: which USC type, and which bill.
    pub document_type: DocumentType,

    /// Source credits and references for this element
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_credits: Vec<SourceCredit>,
}

impl UslmFacts {
    /// Write these facts into a payload, in the namespace of their document type.
    pub fn to_payload(&self) -> Result<ClassPayload, serde_json::Error> {
        ClassPayload::of(self.document_type.namespace(), self)
    }

    /// Read the USLM facts of a node, or `None` if it is not a USLM node.
    ///
    /// `None` for a node of another class, and for a USLM node whose payload will
    /// not parse. Both are the same answer to the caller: this reader cannot
    /// speak for this node.
    pub fn of(data: &NodeData) -> Option<Self> {
        let payload = data
            .payload_in(NodeType::USCODE)
            .or_else(|| data.payload_in(NodeType::BILL))?;
        payload.read().ok()
    }
}
