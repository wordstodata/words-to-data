//! What a citation can point at in this dataset.
//!
//! Two questions, in this order.
//!
//! Does the dataset carry the title at all? If it does not, the answer is
//! [`Resolution::OutOfScope`]. A dataset holding title 26 knows nothing about
//! title 42, and reporting "not found" would tell a researcher that no such
//! authority exists (`src/dataset/scope.rs`).
//!
//! If it does carry the title, where is the section? A structural path is the
//! answer, because a provision has no identity of its own yet
//! (`docs/adr/0001-structural-paths-locate-not-identify.md`). A citation gives a
//! title and a section number, and a structural path also carries the chapters
//! and parts between them — `uscode/title_26/subtitle_A/chapter_1/subchapter_B/
//! part_VI/section_174` — so the path cannot be built from the citation alone.
//! [`SectionPaths`] is the index that closes the gap, built from the works the
//! caller already has in hand.
//!
//! Reading the index from a parsed work, rather than from storage, keeps this
//! module out of the way of the storage trait: a caller that has a dataset, a
//! file, or one title in memory can all resolve the same way.

use std::collections::BTreeMap;

use crate::dataset::{Coverage, Scope};
use crate::document::{DocumentNode, NodeType};
use crate::uslm::{ElementType, UslmFacts};

use super::usc::UscCitation;

/// Whether a node is a section of the US Code, and not of a public law.
///
/// A citation to `26 U.S.C. § 174` names the Code. The node type answers both
/// halves: its namespace says which class of document, and its local name says
/// which kind of element.
fn is_usc_section(node_type: &NodeType) -> bool {
    node_type.namespace() == NodeType::USCODE
        && node_type.local() == ElementType::Section.local_name()
}

/// Where the sections of a dataset are, by USLM identifier.
///
/// The identifier `/us/usc/t26/s174` is what a citation can be turned into
/// without reading anything; the structural path is what the model uses. This
/// maps the first to the second.
#[derive(Debug, Clone, Default)]
pub struct SectionPaths {
    paths: BTreeMap<String, Vec<String>>,
}

impl SectionPaths {
    pub fn new() -> Self {
        Self::default()
    }

    /// Index every section of one parsed work, such as a title of the U.S. Code.
    ///
    /// Call it once per work held. Indexing the same work twice would report its
    /// sections twice.
    pub fn add_work(&mut self, work: &DocumentNode) {
        if is_usc_section(&work.data.node_type)
            && let Some(uslm_id) = UslmFacts::of(&work.data).and_then(|facts| facts.uslm_id)
        {
            self.paths
                .entry(uslm_id)
                .or_default()
                .push(work.data.path.to_string());
        }
        for child in &work.children {
            self.add_work(child);
        }
    }

    /// Every path a USLM identifier names, in document order.
    ///
    /// More than one is possible: the law sometimes numbers two provisions
    /// alike, and the document records both, so reporting only the first would
    /// be a silent loss (`DocumentNode::find_all`).
    pub fn paths_of(&self, uslm_id: &str) -> &[String] {
        self.paths.get(uslm_id).map_or(&[], Vec::as_slice)
    }

    /// How many identifiers are indexed.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

/// What one cited section came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The dataset holds the title, and these paths locate the section in it.
    Provision { paths: Vec<String> },
    /// The dataset holds the title, and it has no section with this number.
    ///
    /// A statement about the law as this dataset records it, not about our
    /// coverage. A citation to a repealed or misprinted section lands here.
    Absent,
    /// The dataset does not carry this title, so it can say nothing about the
    /// section. This is the answer to give, never "not found".
    OutOfScope,
    /// The dataset said it would carry this title and does not: a fault in the
    /// build rather than a statement about the law.
    Gap,
}

/// One section a citation named, and what it came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedSection {
    /// As the citation wrote it: `981(a)(l)(C)`.
    pub section: String,
    /// The section it names: `/us/usc/t18/s981`. A subsection is dropped here,
    /// because the citation cannot be trusted at that depth
    /// ([`UscCitation::uslm_id`]).
    pub uslm_id: String,
    pub resolution: Resolution,
}

/// Resolve every section a citation names.
///
/// One entry per section, in the order the citation wrote them, so a caller can
/// report the whole citation rather than the part of it that resolved.
pub fn resolve(citation: &UscCitation, scope: &Scope, paths: &SectionPaths) -> Vec<CitedSection> {
    let coverage = scope.covers(citation.work().as_str());

    citation
        .sections
        .iter()
        .map(|section| {
            let uslm_id = citation.uslm_id(section);
            let resolution = match coverage {
                Coverage::OutOfScope => Resolution::OutOfScope,
                Coverage::Gap => Resolution::Gap,
                Coverage::InScope => match paths.paths_of(&uslm_id) {
                    [] => Resolution::Absent,
                    found => Resolution::Provision {
                        paths: found.to_vec(),
                    },
                },
            };
            CitedSection {
                section: section.clone(),
                uslm_id,
                resolution,
            }
        })
        .collect()
}
