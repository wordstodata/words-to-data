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
//! ## A case is a work, and each writing is a child
//!
//! A case is one work, `judicial/case_<cluster>`, whose root carries the
//! [`CaseFacts`]. Each writing — the Court's opinion, a concurrence, a dissent —
//! is a child, `opinion_<id>`, whose node carries the [`OpinionFacts`] and the
//! whole text of the writing
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`).
//! Nothing has parsed a writing's own pages and paragraphs yet (#314).
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

pub mod cites;
pub mod held;
pub mod reliance;

use serde::{Deserialize, Serialize};

use crate::document::{ClassPayload, NodeData, NodeType};

/// What a judicial reader knows about a case that the core does not: the facts
/// of the cluster, which belong to the case and not to any one writing.
///
/// Before you add a field here, work the [review checklist on
/// `ClassPayload`](crate::document::ClassPayload#review-checklist-before-you-add-a-field-to-a-payload-type).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CaseFacts {
    /// The case as it is cited: `Obergefell v. Hodges`.
    pub case_name: String,

    /// The publisher's id for the case: CourtListener's cluster id. The work is
    /// named by it, `judicial/case_<cluster>`.
    pub cluster_id: u64,

    /// The judges who sat, in the publisher's own words and punctuation.
    ///
    /// One string rather than a list, because the source gives one string and
    /// splitting it would invent a boundary. `Kennedy` is a bench of one;
    /// `Higginbotham, Owen and Haynes` is a bench of three.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,

    /// Whether the case is precedent: `Published`, `Unpublished`, and the other
    /// values the publisher uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedential_status: Option<String>,

    /// The parallel citations, as the reporters print them:
    /// `576 U.S. 644`, `135 S. Ct. 2584`. A reporter citation names the case,
    /// not one writing of it.
    ///
    /// These are citations *of* this case, not citations *by* it: what a writing
    /// cites is a `judicial.cites` link and needs no payload (#52).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<String>,

    /// Where the publisher says the record came from, in its own codes, such as
    /// CourtListener's `CU`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// The docket the case sat on, which is the next step towards the court.
    ///
    /// The court's own name is one request further along and is not held here:
    /// a budget of 125 requests a day was spent on opinions and cases instead
    /// (#53).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docket_id: Option<u64>,

    /// The publisher's combined records of this case that are **not** stored as
    /// text: records that hold, in one text, the same words as the split records
    /// stored as the writings. Kept so a reader can go and look at them, and so
    /// nobody takes their absence for a loss
    /// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
    /// decision 2).
    ///
    /// Here and not in the core: it does not say how any stored text was
    /// obtained — each writing's own provenance says that — and no reader
    /// outside the judicial class reads it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub combined_records: Vec<u64>,
}

impl CaseFacts {
    /// Write these facts into a payload in the `judicial` namespace.
    pub fn to_payload(&self) -> Result<ClassPayload, serde_json::Error> {
        ClassPayload::of(NodeType::JUDICIAL, self)
    }

    /// Read the case facts of a node, or `None` if it is not a case.
    pub fn of(data: &NodeData) -> Option<Self> {
        if data.node_type.as_str() != NodeType::CASE {
            return None;
        }
        data.payload_in(NodeType::JUDICIAL)?.read().ok()
    }
}

/// What a judicial reader knows about one writing of a case that the core does
/// not.
///
/// Nothing here is needed in order to *report* the node. Notably absent is
/// whether the text came from OCR: that says how the text was obtained rather
/// than anything about the court, so it belongs in the node's [`Provenance`] as a
/// [`text_method`], where a reader deciding whether to trust a passage finds it
/// without opening a payload. The case name and the reporter citations are on
/// the case, in [`CaseFacts`].
///
/// Before you add a field here, work the [review checklist on
/// `ClassPayload`](crate::document::ClassPayload#review-checklist-before-you-add-a-field-to-a-payload-type):
/// anything a reader needs in order to report the node belongs in the core.
///
/// [`Provenance`]: crate::link::Provenance
/// [`text_method`]: crate::document::text_method
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpinionFacts {
    /// What kind of writing this is, in the publisher's own vocabulary, such as
    /// CourtListener's `020lead`, `040dissent`, or `010combined` for one record
    /// that holds every writing of the case. Kept verbatim rather than mapped
    /// onto an enum of ours: a vocabulary we do not own is not ours to close.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opinion_type: Option<String>,

    /// The judge the publisher names as its author, when it names one.
    ///
    /// `None` covers a per curiam opinion and a record where nobody is named,
    /// which are different things. [`OpinionFacts::per_curiam`] is what tells
    /// them apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Whether the publisher says the court issued this per curiam — in the name
    /// of the court, with no judge named as its author.
    ///
    /// Kept beside `author` because without it an unattributed opinion and an
    /// opinion nobody recorded an author for arrive as the same `None`, and only
    /// one of those is a fact about the court.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_curiam: Option<bool>,

    /// The content hash the publisher supplies for the text it sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,

    /// How many pages the printed opinion runs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,

    /// Where the publisher says it got the document, such as a court's own PDF.
    ///
    /// This is what makes "the text came from a court PDF" something a reader can
    /// check rather than take on trust. It is not needed in order to *report* the
    /// node — the node's [`Provenance`] already says the text came from a text
    /// layer — so it belongs here rather than in the core
    /// (`docs/adr/0006-a-document-node-is-class-neutral.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
}

impl OpinionFacts {
    /// Write these facts into a payload in the `judicial` namespace.
    pub fn to_payload(&self) -> Result<ClassPayload, serde_json::Error> {
        ClassPayload::of(NodeType::JUDICIAL, self)
    }

    /// Read the facts of a writing, or `None` if the node is not a writing.
    pub fn of(data: &NodeData) -> Option<Self> {
        if data.node_type.as_str() != NodeType::OPINION {
            return None;
        }
        data.payload_in(NodeType::JUDICIAL)?.read().ok()
    }
}
