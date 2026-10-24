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

use serde::Serialize;

use crate::dataset::{Coverage, Scope};
use crate::document::{DocumentNode, NodeData, NodeType};
use crate::legislature::redesignation::{Step, walk_down};
use crate::uslm::{ElementType, UslmFacts};

use super::usc::{UscCitation, fold_dashes, range_ends, subdivisions};

/// Whether a node is a section of the US Code, and not of a public law.
///
/// A citation to `26 U.S.C. § 174` names the Code. The node type answers both
/// halves: its namespace says which class of document, and its local name says
/// which kind of element.
fn is_usc_section(node_type: &NodeType) -> bool {
    node_type.namespace() == NodeType::USCODE
        && node_type.local() == ElementType::Section.type_name()
}

/// Where the sections of a dataset are, by USLM identifier.
///
/// The identifier `/us/usc/t26/s174` is what a citation can be turned into
/// without reading anything; the structural path is what the model uses. This
/// maps the first to the second.
///
/// The key is the identifier with its dashes folded
/// ([`fold_dashes`](super::usc::fold_dashes)), because the publisher writes a
/// dash in a section number as an en dash and prose writes a hyphen. Both sides
/// of the comparison are folded — the key stored here and the key a citation is
/// looked up by — so one spelling finds the other (#141). Nothing stored is
/// touched: the values are the paths as the build wrote them, en dashes and all.
#[derive(Debug, Clone, Default)]
pub struct SectionPaths {
    paths: BTreeMap<String, Vec<String>>,
    /// The heading of every indexed section that is a stub, by path
    /// ([`stub_heading`]).
    stubs: BTreeMap<String, String>,
    /// The shape of every indexed section, by path: its nodes and their paths,
    /// with no words. Enough to walk a written subdivision down to the
    /// provision it names ([`reach`]), and small: a title of the Code is tens of
    /// thousands of nodes, and their words are most of its size.
    shapes: BTreeMap<String, DocumentNode>,
    /// The date of the printing indexed, when the caller named it
    /// ([`SectionPaths::of_printing`]).
    printed: Option<String>,
}

impl SectionPaths {
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty index of the printing dated `date`. A section resolved in it
    /// says so ([`CitedSection::resolved_in`], #317).
    pub fn of_printing(date: impl Into<String>) -> Self {
        Self {
            printed: Some(date.into()),
            ..Self::default()
        }
    }

    /// The date of the printing indexed, when the caller named it.
    pub fn printed(&self) -> Option<&str> {
        self.printed.as_deref()
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
                .entry(fold_dashes(&uslm_id))
                .or_default()
                .push(work.data.path.to_string());
            if let Some(heading) = stub_heading(work) {
                self.stubs.insert(work.data.path.to_string(), heading);
            }
            self.shapes
                .insert(work.data.path.to_string(), shape_of(work));
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
    ///
    /// The identifier asked for is folded the same way the keys are, so a
    /// citation may spell a dash either way.
    pub fn paths_of(&self, uslm_id: &str) -> &[String] {
        self.paths
            .get(&fold_dashes(uslm_id))
            .map_or(&[], Vec::as_slice)
    }

    /// The stub heading of the section at `path`, such as `[Renumbered § 30]`,
    /// when the indexed work prints it as a stub ([`stub_heading`]).
    pub fn stub_at(&self, path: &str) -> Option<&str> {
        self.stubs.get(path).map(String::as_str)
    }

    /// Where the subdivision written as `written` leads below the section at
    /// `path` ([`reach`]). The section itself, with nothing unresolved, when the
    /// index does not hold the section's shape.
    pub fn reach(&self, path: &str, written: &str) -> Reached {
        match self.shapes.get(path) {
            Some(section) => reach(section, written),
            None => Reached {
                path: path.to_string(),
                unresolved: None,
            },
        }
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
    /// Where the subdivision written leads below each path of
    /// [`Resolution::Provision`], in the same order. Empty for a section that
    /// did not resolve.
    pub reached: Vec<Reached>,
    /// The date of the printing the section was resolved in, when the index
    /// names one ([`SectionPaths::of_printing`]).
    pub resolved_in: Option<String>,
}

/// The deepest provision a written subdivision leads to, and the part of it
/// that leads nowhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reached {
    /// The deepest path the written designations lead to: the section when the
    /// citation writes none, or when the first of them leads nowhere.
    pub path: String,
    /// The designations from the first one that leads nowhere, as written:
    /// `(e)` of `174(e)`, `(l)(B)(i)` of `41(d)(l)(B)(i)`. `None` when every
    /// designation leads to a provision.
    pub unresolved: Option<String>,
}

/// Resolve every section a citation names.
///
/// One entry per section, in the order the citation wrote them, so a caller can
/// report the whole citation rather than the part of it that resolved.
///
/// A range is two entries. `26 U.S.C. §§ 991-997` names no section numbered
/// `991-997`, and its number can be a range ([`range_ends`]), so each end is
/// resolved on its own and both entries keep the section as written (#309). The
/// sections between the ends are not named: the text did not write them, and a
/// link says what the text wrote. A number the title does hold, such as
/// `20 U.S.C. § 1078-1`, is that section and never a range.
pub fn resolve(citation: &UscCitation, scope: &Scope, paths: &SectionPaths) -> Vec<CitedSection> {
    let coverage = scope.covers(citation.work().as_str());
    let cited = |section: &String, number: &str| {
        let uslm_id = citation.uslm_id(number);
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
        let reached = match &resolution {
            Resolution::Provision { paths: found } => found
                .iter()
                .map(|path| paths.reach(path, section))
                .collect(),
            _ => Vec::new(),
        };
        CitedSection {
            section: section.clone(),
            uslm_id,
            resolution,
            reached,
            resolved_in: paths.printed.clone(),
        }
    };

    citation
        .sections
        .iter()
        .flat_map(|section| {
            let whole = cited(section, section);
            match (&whole.resolution, range_ends(section)) {
                (Resolution::Absent, Some((from, to))) => {
                    vec![cited(section, &from), cited(section, &to)]
                }
                _ => vec![whole],
            }
        })
        .collect()
}

/// Where the subdivision a citation wrote leads, below the section it names.
///
/// Serialized with a `resolution` tag, so a reader of JSON is told whether the
/// path was resolved or the citation fell back to the section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "resolution", rename_all = "snake_case")]
pub enum Subdivision {
    /// The citation names the section alone: `26 U.S.C. § 174`.
    Section,
    /// Every designation leads to a provision, and this is its path.
    Resolved { resolved_path: String },
    /// A designation leads to nothing in this printing. `unresolved` is that
    /// designation as written, `(e)` for `174(e)`, so a reader is told which part
    /// failed and not only that something did.
    Unresolved { unresolved: String },
}

/// Resolve the subdivision a citation wrote, such as the `(a)(1)` of
/// `174(a)(1)`, below one printing of its section.
///
/// A link ends at the subdivision it resolved to in the printing it was made
/// against ([`reach`], #316). A reader resolves the subdivision again against
/// the printing it is looking at, and writes nothing. A designation the
/// printing does not hold is reported by name rather than guessed at: the
/// published example `981(a)(l)(C)` has a lower-case L where the provision has
/// a paragraph `(1)`.
///
/// The walk is the one an amendment's address takes below its section, so a
/// citation and an amendment resolve a designation alike.
pub fn resolve_subdivision(section: &DocumentNode, written: &str) -> Subdivision {
    if subdivisions(written).is_empty() {
        return Subdivision::Section;
    }
    match reach(section, written) {
        Reached {
            unresolved: Some(unresolved),
            ..
        } => Subdivision::Unresolved { unresolved },
        Reached { path, .. } => Subdivision::Resolved {
            resolved_path: path,
        },
    }
}

/// Where the subdivision a citation wrote leads below one printing of its
/// section: the deepest provision its designations name, and the designations
/// from the first one that names nothing (ADR 0014, decision 6). `41(d)(l)(B)(i)`
/// leads to (d), and `(l)(B)(i)` is the part that leads nowhere: the `(l)` is an
/// OCR misreading of `(1)`, and it is named, never repaired.
///
/// The walk is the one an amendment's address takes below its section, so a
/// citation and an amendment resolve a designation alike.
pub fn reach(section: &DocumentNode, written: &str) -> Reached {
    let designations = subdivisions(written);
    let mut current = section;
    for (index, designation) in designations.iter().enumerate() {
        match walk_down(current, &[Step::numbered(*designation)]) {
            Ok(next) => current = next,
            Err(_) => {
                return Reached {
                    path: current.data.path.to_string(),
                    unresolved: Some(
                        designations[index..]
                            .iter()
                            .map(|part| format!("({part})"))
                            .collect(),
                    ),
                };
            }
        }
    }
    Reached {
        path: current.data.path.to_string(),
        unresolved: None,
    }
}

/// The shape of a section: each node with its path and type, and no words.
fn shape_of(node: &DocumentNode) -> DocumentNode {
    DocumentNode {
        data: NodeData::new(
            node.data.path.clone(),
            node.data.node_type.clone(),
            node.data.date,
        ),
        children: node.children.iter().map(shape_of).collect(),
    }
}

/// The words a stub's heading starts with: the section is no longer law, and
/// says where it went, or that it went.
const STUB_WORDS: [&str; 4] = ["Repealed", "Renumbered", "Transferred", "Omitted"];

/// The heading of a section that is now a stub, written as the Code prints it:
/// `[Renumbered § 30]` for `26 U.S.C. § 44F`. `None` for a section of law.
///
/// A stub is a section whose heading says it was Repealed, Renumbered,
/// Transferred or Omitted, inside the brackets the Code prints around it:
/// `[§ 44F. Renumbered § 30]`. The number carries the opening bracket and the
/// heading the closing one, so the heading must end in `]`. That keeps out a
/// section of law whose heading starts with one of the words, such as one
/// headed "Transferred functions".
///
/// A citation that resolves to a stub resolves to a real node, and the link
/// looked as good as any other (#310). It is not: the court cited law that
/// this printing no longer holds at that number. Readers say so with the
/// heading, and this is the one place that decides what a stub is.
///
/// The rule reads the node of one printing, so the answer is about that
/// printing: § 44F was law when *United Stationers* cited it in 1997.
pub fn stub_heading(section: &DocumentNode) -> Option<String> {
    let heading = section.data.heading.as_deref()?.trim();
    let first_word = heading
        .trim_start_matches('[')
        .split(|c: char| !c.is_alphabetic())
        .next()?;
    if !heading.ends_with(']') || !STUB_WORDS.contains(&first_word) {
        return None;
    }
    match heading.starts_with('[') {
        true => Some(heading.to_string()),
        false => Some(format!("[{heading}")),
    }
}
