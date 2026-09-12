//! Court material: the facts about an opinion that only a judicial reader knows.
//!
//! An opinion is a [`DocumentNode`] like any other. Its path, its date, its text
//! and its provenance are core, so a reader that has never heard of a court can
//! still search it, diff it and quote it. What only a judicial reader
//! understands — the case name, who wrote it, what the reporters call it, whether
//! it is precedent — travels beside the node in a [`ClassPayload`]
//! (`docs/adr/0006-a-document-node-is-class-neutral.md`).
//!
//! [`DocumentNode`]: crate::document::DocumentNode
//!
//! ## An opinion is one node
//!
//! It has no children today. The structure is in the text, and nothing has parsed
//! it: a CourtListener record carries `xml_harvard` with `<opinion type="majority">`
//! and `<author>` inside it, and that is deliberately passed over for now. The
//! node gains children later and its type does not change (#53).
//!
//! ## The date is not on the opinion
//!
//! An expression is one work as it read on one date, and for an opinion that date
//! is the day the court filed it. A CourtListener opinion record carries
//! `date_created` and `date_modified`, and **neither is it**: they say when
//! CourtListener ingested and last touched the record. The filing date is
//! `date_filed` on the *cluster*, which is a second request. Using `date_created`
//! would date every opinion to the day a third party scraped it, and it is the
//! field that looks right.

use serde::{Deserialize, Serialize};

use crate::document::{ClassPayload, NodeData, NodeType};

/// What a judicial reader knows about an opinion that the core does not.
///
/// Nothing here is needed in order to *report* the node. Notably absent is
/// whether the text came from OCR: that says how the text was obtained rather
/// than anything about the court, so it belongs in the node's [`Provenance`] as a
/// [`text_method`], where a reader deciding whether to trust a passage finds it
/// without opening a payload.
///
/// [`Provenance`]: crate::link::Provenance
/// [`text_method`]: crate::document::text_method
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpinionFacts {
    /// The case as it is cited: `Obergefell v. Hodges`.
    pub case_name: String,

    /// What kind of opinion this is, in the publisher's own vocabulary, such as
    /// CourtListener's `010combined`. Kept verbatim rather than mapped onto an
    /// enum of ours: a vocabulary we do not own is not ours to close.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opinion_type: Option<String>,

    /// The judge the publisher names as its author, when it names one.
    ///
    /// `None` covers a per curiam opinion and a record where nobody is named,
    /// which are different things; see `per_curiam` in the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Whether the opinion is precedent: `Published`, `Unpublished`, and the
    /// other values the publisher uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedential_status: Option<String>,

    /// The parallel citations, as the reporters print them:
    /// `576 U.S. 644`, `135 S. Ct. 2584`.
    ///
    /// A list because one opinion has several, and they are how a lawyer names
    /// it. These are citations *of* this opinion, not citations *by* it: what the
    /// opinion cites is a `judicial.cites` link and needs no payload (#52).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<String>,

    /// Where the publisher says the record came from, in its own codes, such as
    /// CourtListener's `CU`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// The content hash the publisher supplies for the text it sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,

    /// How many pages the printed opinion runs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
}

impl OpinionFacts {
    /// Write these facts into a payload in the `judicial` namespace.
    pub fn to_payload(&self) -> Result<ClassPayload, serde_json::Error> {
        ClassPayload::of(NodeType::JUDICIAL, self)
    }

    /// Read the judicial facts of a node, or `None` if it is not a judicial node.
    pub fn of(data: &NodeData) -> Option<Self> {
        data.payload_in(NodeType::JUDICIAL)?.read().ok()
    }
}
