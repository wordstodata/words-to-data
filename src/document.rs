//! The document tree: one node, in any document class.
//!
//! A [`DocumentNode`] carries what any reader needs in order to locate, search,
//! diff and report a piece of a document: a structural path, a type, a date, the
//! five text fields, its children, and where its text came from. It carries
//! nothing that belongs to one class of document.
//!
//! This tree used to be `USLMElement`, named after one publisher's XML schema,
//! and every node had to declare a `DocumentType` from a closed set of US Code
//! or bill. A court opinion could be stored only by saying it was one of those
//! two, which is a confident wrong answer in a file sent to another party
//! (#129). The US Code is now one tenant of this tree rather than its shape.
//!
//! The facts that only one class understands travel beside the node in a
//! [`ClassPayload`], on the same contract a link's payload already has: the core
//! stores it, hands it back unchanged, and never reads it
//! (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`,
//! `docs/adr/0006-a-document-node-is-class-neutral.md`).
//!
//! Two rules keep the payload from becoming a dumping ground. The core never
//! reads it. And nothing a reader needs in order to *report* a node may live
//! there: a reader that cannot open the payload must still be able to say where
//! the node is, what kind of thing it is, what it says, and how its text was
//! obtained.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::Date;

use crate::intern::StringInterner;
use crate::link::Provenance;

/// What kind of thing a node is, namespaced by the class that defines it.
///
/// A namespaced open string rather than an enum we control, exactly as
/// [`LinkKind`] already is. The core reads the type — the diff refuses to pair
/// two nodes of different types, and a reader reports it — but the core does not
/// own the vocabulary, so another party can add a document class without our
/// permission (`docs/adr/0002-links-live-in-the-core.md`).
///
/// The namespace names the class: `uscode.section` is a section of the US Code,
/// `public_law.section` is a section of a public law, and `judicial.opinion` is
/// a court opinion. A type this build has never seen is carried unchanged rather
/// than dropped, because silent loss is the one failure a portable format cannot
/// have.
///
/// [`LinkKind`]: crate::link::LinkKind
///
/// # Examples
///
/// ```
/// use words_to_data::document::NodeType;
///
/// let section = NodeType::new("uscode.section");
/// assert_eq!(section.namespace(), "uscode");
/// assert_eq!(section.local(), "section");
///
/// // A class this build knows nothing about still answers both questions.
/// let headnote = NodeType::new("westlaw.headnote");
/// assert_eq!(headnote.namespace(), "westlaw");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeType(pub String);

impl NodeType {
    /// The US Code. `uscode.section`, `uscode.subsection`.
    pub const USCODE: &'static str = "uscode";

    /// A public law. The same USLM vocabulary as the US Code below the root, in
    /// its own namespace, because a public law is not part of the US Code and
    /// nothing stored may say that it is.
    pub const PUBLIC_LAW: &'static str = "public_law";

    /// Court material. `judicial.opinion`.
    pub const JUDICIAL: &'static str = "judicial";

    /// One court opinion, whole. An opinion is a single node today; it gains
    /// children later and the type does not change (#53).
    pub const OPINION: &'static str = "judicial.opinion";

    pub fn new(node_type: impl Into<String>) -> Self {
        Self(node_type.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The namespace half, `uscode` in `uscode.section`.
    ///
    /// A reader uses this to decide whether it understands the class. It may
    /// not, and that is allowed: it still carries the node.
    pub fn namespace(&self) -> &str {
        self.0.split_once('.').map_or(&self.0, |(before, _)| before)
    }

    /// The local half, `section` in `uscode.section`.
    ///
    /// The class's own name for this kind of node, which for a USLM document is
    /// the publisher's element name.
    pub fn local(&self) -> &str {
        self.0.split_once('.').map_or(&self.0, |(_, after)| after)
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a node's text was obtained, for [`Provenance::method`].
///
/// Core provenance rather than a class payload. A reader deciding whether to
/// trust a passage must not have to open an extension payload to learn that the
/// text came from a machine reading a scan (#129).
pub mod text_method {
    /// Optical character recognition. Nobody has checked the result against the
    /// page, so the words may not be the words the court wrote.
    pub const OCR: &str = "ocr";

    /// Read from a text layer the publisher supplied.
    pub const TEXT_LAYER: &str = "text_layer";

    /// Read from the publisher's own structured markup, such as USLM XML.
    pub const MARKUP: &str = "markup";
}

/// Facts about a node that only the class defining its type understands.
///
/// The core stores it, hands it back unchanged, and never reads it. The same
/// contract, and the same two rules, as a link's kind payload
/// (`crate::link::KindPayload`).
///
/// The facts are held as JSON **text**, not as a parsed value tree. Two reasons,
/// and both follow from "hands it back unchanged": text is returned byte for
/// byte, while a value tree reorders an object's keys on the way through; and a
/// corpus holds millions of nodes, where a map per node costs far more memory
/// than one string. The core never looks inside either way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassPayload {
    /// The namespace that owns these facts, such as `uscode` or `judicial`.
    pub namespace: Arc<str>,
    /// The facts themselves as JSON text, opaque to the core.
    pub value: Arc<str>,
}

impl ClassPayload {
    /// Write a class's facts into a payload.
    ///
    /// Going through a type, rather than building JSON by hand, is what keeps
    /// the payload a record of facts rather than a bag of strings. The class owns
    /// the type; the core only ever sees the text it produced.
    pub fn of<T: Serialize>(
        namespace: impl Into<Arc<str>>,
        facts: &T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            namespace: namespace.into(),
            value: serde_json::to_string(facts)?.into(),
        })
    }

    /// Read a class's facts back out of a payload.
    pub fn read<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.value)
    }
}

/// The different text content fields that can be present in a document node
///
/// A node can have up to five distinct text fields, each serving a specific
/// purpose in the document structure. These fields are tracked separately to
/// enable precise change detection when comparing document versions.
///
/// - Heading: Opening text that appears before enumerated sub-elements
/// - Chapeau: A conditional or qualifying clause (often starting with "Provided that")
/// - Proviso: The main text content of the element
/// - Content: Text that appears after all child elements
/// - Continuation: Text that appears after all child elements
///
/// **IMPORTANT**: Because continuations appear _after_ child nodes, the full text of some nodes require child nodes to be present. This makes sense, to load a full section, you need the subsections, which need paragraphs which may need clauses, etc.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextContentField {
    /// The heading or title text of the node (e.g., "Agricultural Programs",
    /// or the case name of an opinion)
    Heading,
    /// Opening text that appears before enumerated sub-elements
    Chapeau,
    /// A conditional or qualifying clause (often starting with "Provided that")
    Proviso,
    /// The main text content of the node
    Content,
    /// Text content that appears after all child nodes
    Continuation,
}

/// Everything the core knows about one node of a document.
///
/// # Path
///
/// The structural path locates the node in the hierarchy the parser found:
/// `uscode/title_26/subtitle_k/chapter_100/section_9834/level_1`. Combining it
/// with [`NodeData::date`] locates one node in one printing of one document. It
/// locates rather than identifies
/// (`docs/adr/0001-structural-paths-locate-not-identify.md`).
///
/// # What is not here
///
/// A USLM identifier, an element's number, a case name, an opinion's author. All
/// of those belong to one class of document, and they travel in
/// [`NodeData::payload`]. `crate::uslm::UslmFacts` reads the US Code's, and
/// `crate::judicial::OpinionFacts` reads an opinion's.
// No `Eq` or `Hash`: a node can carry a `Provenance`, which holds a raw model
// score as an `f32`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeData {
    /// The full structural path of this node in its document.
    pub path: Arc<str>,

    /// What kind of thing this node is, such as `uscode.section`.
    pub node_type: NodeType,

    /// The date this version of the document was published.
    ///
    /// For a statute, the release point. For a document published once and never
    /// amended, the date it was published — for a court opinion, the date the
    /// court filed it, never the date a third party ingested the record.
    pub date: Date,

    // Content fields. These are the fields the diff runs on, and the fields
    // search reads. They are core because every document class has text.
    /// The heading or title text of the node
    pub heading: Option<Arc<str>>,

    /// The words at the start of the node that appear before any enumerated items
    pub chapeau: Option<Arc<str>>,

    /// A clause imposing a qualification, condition, or restriction
    pub proviso: Option<Arc<str>>,

    /// The main text content of the node
    pub content: Option<Arc<str>>,

    /// Text content that appears after all child nodes
    pub continuation: Option<Arc<str>>,

    /// Where this node's text came from, when the producer recorded it.
    ///
    /// Core, not a payload fact. `method` says *how* the text was obtained —
    /// see [`text_method`] — and a reader judging whether to rely on a passage
    /// needs that without opening an extension payload. `None` means the
    /// producer recorded nothing, which is every node written before this
    /// existed.
    ///
    /// Boxed because most nodes carry none. A [`Provenance`] held inline would
    /// spend its whole size on every node in the corpus to say "nothing here".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Box<Provenance>>,

    /// Facts only this node's class understands.
    ///
    /// `None` for a class that needs nothing beyond the core. The core never
    /// reads what is here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<ClassPayload>,
}

impl NodeData {
    /// A node with a path, a type and a date, and no text.
    ///
    /// The five text fields, the provenance and the payload are set by the
    /// producer. A constructor rather than `Default` because a node with no path
    /// and no date is not a node.
    pub fn new(path: impl Into<Arc<str>>, node_type: NodeType, date: Date) -> Self {
        Self {
            path: path.into(),
            node_type,
            date,
            heading: None,
            chapeau: None,
            proviso: None,
            content: None,
            continuation: None,
            provenance: None,
            payload: None,
        }
    }

    /// Record where this node's text came from.
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(Box::new(provenance));
        self
    }

    /// Attach the facts only this node's class understands.
    pub fn with_payload(mut self, payload: ClassPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Retrieve the text content for a specific field
    ///
    /// Returns `Some` if the field has content, or `None` if the field is empty
    /// for this node.
    pub fn get_text_content(&self, field: TextContentField) -> Option<Arc<str>> {
        match field {
            TextContentField::Heading => self.heading.clone(),
            TextContentField::Chapeau => self.chapeau.clone(),
            TextContentField::Proviso => self.proviso.clone(),
            TextContentField::Content => self.content.clone(),
            TextContentField::Continuation => self.continuation.clone(),
        }
    }

    /// The class payload, when it was written in `namespace`.
    ///
    /// The one place a caller is meant to reach a payload from. Asking for a
    /// namespace, rather than taking whatever is there, is what stops one
    /// class's reader from parsing another class's facts.
    pub fn payload_in(&self, namespace: &str) -> Option<&ClassPayload> {
        self.payload
            .as_ref()
            .filter(|payload| &*payload.namespace == namespace)
    }

    pub fn intern_strings(&mut self, interner: &mut StringInterner) {
        self.path = interner.intern(&self.path);
        // The namespace only: one of a handful of values, repeated over the
        // whole corpus. The facts beside it are different on every node, so
        // interning them would cost a lookup per node and share nothing.
        if let Some(payload) = &mut self.payload {
            payload.namespace = interner.intern(&payload.namespace);
        }

        self.heading = interner.intern_option(&self.heading);
        self.chapeau = interner.intern_option(&self.chapeau);
        self.proviso = interner.intern_option(&self.proviso);
        self.content = interner.intern_option(&self.content);
        self.continuation = interner.intern_option(&self.continuation);
    }
}

/// One node of a document tree, with its children in document order.
///
/// # Examples
///
/// A USC section reads as:
///
/// ```text
/// Section 174 (DocumentNode)
///   ├─ data: NodeData { node_type: uscode.section, heading: "Research expenditures", ... }
///   └─ children:
///       ├─ Subsection (a) (DocumentNode)
///       │   └─ children: [Paragraph (1), Paragraph (2), ...]
///       └─ Subsection (b) (DocumentNode)
///           └─ children: [...]
/// ```
///
/// A court opinion reads as one node with no children, which is ordinary rather
/// than degenerate: the structure is in the text and nothing has parsed it yet.
///
/// Use [`DocumentNode::find`] to locate a node in the tree by its structural
/// path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DocumentNode {
    /// The metadata and content for this node
    pub data: NodeData,

    /// Child nodes in document order
    pub children: Vec<DocumentNode>,
}

impl DocumentNode {
    /// A leaf node: one node with no children.
    ///
    /// The whole of a document that has not been taken apart, such as a court
    /// opinion.
    pub fn leaf(data: NodeData) -> Self {
        Self {
            data,
            children: Vec::new(),
        }
    }

    /// Search for a node by its structural path
    ///
    /// Recursively searches this node and all descendants for a node with the
    /// specified path. The path must be a fully qualified structural path
    /// (e.g., "uscode/title_7/chapter_1/section_1").
    ///
    /// # Returns
    ///
    /// Returns `Some(&DocumentNode)` if a node with the matching path is found,
    /// or `None` if no such node exists in this tree.
    ///
    /// # Examples
    ///
    /// ```
    /// # use words_to_data::uslm::parser::parse;
    /// # let node = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    /// // Find a specific section
    /// let section = node.find("uscode/title_7/chapter_1/section_2");
    /// assert!(section.is_some());
    ///
    /// // Non-existent path returns None
    /// let missing = node.find("uscode/title_99");
    /// assert!(missing.is_none());
    /// ```
    pub fn find(&self, path: &str) -> Option<&DocumentNode> {
        self.find_all(path).into_iter().next()
    }

    /// Every node at this structural path, in document order
    ///
    /// A path can name more than one provision: the law sometimes numbers two
    /// provisions alike, and the document records both. `26 U.S.C. § 45X(d)(4)`
    /// is two paragraphs (4), and the U.S. Code renders both. Prefer this over
    /// [`DocumentNode::find`] wherever discarding the others would be a silent
    /// loss rather than a convenience.
    ///
    /// # Examples
    ///
    /// ```
    /// # use words_to_data::uslm::parser::parse;
    /// # let node = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    /// let found = node.find_all("uscode/title_7/chapter_1/section_2");
    /// assert_eq!(found.len(), 1);
    /// ```
    pub fn find_all(&self, path: &str) -> Vec<&DocumentNode> {
        if *path == *self.data.path {
            return vec![self];
        }
        // A near miss is a question with an answer. Requiring the separator
        // keeps `uscode/title_26x` from reading as a descendant of
        // `uscode/title_26`, and leaves nothing to assert about.
        let Some(remaining) = path
            .strip_prefix(self.data.path.as_ref())
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return Vec::new();
        };

        let segment = remaining.split('/').next().unwrap_or(remaining);
        let child_path = format!("{}/{segment}", self.data.path);

        self.children
            .iter()
            // Compare the whole path, not a suffix of it: `ends_with` asks
            // whether a child's address happens to end this way, which is a
            // different question from whether it is the child named here.
            .filter(|child| *child.data.path == *child_path)
            .flat_map(|child| child.find_all(path))
            .collect()
    }

    /// Merge the children of one node into another
    pub fn merge_children_mut(&mut self, other: &mut DocumentNode) {
        self.children.append(&mut other.children);
    }

    pub fn intern_strings(&mut self, interner: &mut StringInterner) {
        self.data.intern_strings(interner);
        for child in self.children.iter_mut() {
            child.intern_strings(interner);
        }
    }
}
