//! Storage backends for dataset persistence.
//!
//! The traits split three ways, which is the shape the data model needs (#51):
//!
//! - **Documents** are the core. Expressions, navigation, diffing, and search
//!   work the same whether the document is a statute, a regulation, or an
//!   opinion. Nothing here names a legislature or a court.
//! - **Links** are also core. A link connects a provision to something else and
//!   carries its provenance. Today the only kind is the change annotation, which
//!   ties a change to the amendment that caused it.
//! - **Legislature** is an extension. Bills, sponsors, members, and votes exist
//!   only in datasets that hold legislative material. A dataset of court
//!   opinions has none of them, and must not have to pretend otherwise.
//!
//! `Storage` is the full set that both first-party backends implement today. A
//! backend that holds only documents implements [`DocumentReader`] alone.
//!
//! The unit a backend stores is an **expression**: one work as it read on one
//! date, with its own tree. There is no table of release dates above it, so
//! documents that share no publication cycle can live in one dataset
//! (`docs/adr/0003-storage-is-keyed-by-work.md`).

pub mod memory;
pub mod sqlite;

pub use memory::InMemoryStorage;
pub use sqlite::SqliteStorage;

use std::collections::BTreeMap;

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, HouseRollCall, Member, SponsorInfo, VotePosition};
use crate::dataset::{
    DatasetError, DatasetMetadata, Expression, ExpressionId, ExpressionInfo, SearchResult, WorkId,
};
use crate::diff::TreeDiff;
use crate::link::Link;
use crate::uslm::USLMElement;
use crate::uslm::bill_parser::Bill;

/// The dataset shape this build reads and writes.
///
/// Datasets are rebuilt rather than migrated, so a schema change is a clean
/// break. Both on-disk forms carry this number and refuse a file that does not
/// match, because a break that is not loud reads as an empty dataset.
///
/// 8 gives a container that carries no number a readable path segment, from the
/// publisher's `identifier` or heading in place of an XML uuid, and holds the
/// Federal Rules of Evidence, which `<article>` grouped under a name the parser
/// did not know (#115, #122). No column changes. The number still goes up: a
/// path is what `element_index` is keyed on and what `Target::Provision` names,
/// so a dataset built with uuid paths, read by a build that generates readable
/// ones, would find nothing and say nothing.
/// 7 dates a member's party: the whole party history is kept, in place of the
/// one undated field that reported a 2025 vote through a 2026 affiliation
/// (#105).
/// 6 keeps the verbatim model reply as evidence, stored once under the hash of
/// its own text and referenced from a statement's provenance (#58).
/// 5 stores links directly: one table for every kind, including kinds this
/// build has never seen, with `ChangeAnnotation` projected out of them rather
/// than stored (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
/// 4 added the declared scope; 3 was the work-scoped schema
/// (`docs/adr/0003-storage-is-keyed-by-work.md`).
///
/// One number covering both forms cannot describe a change to only one of
/// them. Widening the SQLite search index (#82) left the compact JSON
/// untouched, because its element trees always carried every text field, so
/// bumping this would have rejected valid JSON datasets to fix a SQLite table.
/// That case is caught where it happens, when the database is opened, rather
/// than here. A change that alters both forms still belongs to this number.
pub const SCHEMA_VERSION: i32 = 8;

/// Reading the documents a dataset holds.
///
/// This is the core interface. Every document class supports it, so nothing
/// here may name a legislature, a court, or any other extension concept.
///
/// It speaks in works and expressions rather than dates. A date alone cannot
/// name anything in a dataset whose documents share no release cycle, which is
/// every dataset except a statutory one.
pub trait DocumentReader {
    /// What this dataset says about itself, including any declared scope.
    ///
    /// Reading metadata is a read. It sat on [`DocumentWriter`] until a scope
    /// needed the declaration, which put a fact the reader depends on behind a
    /// trait a reader has no reason to implement.
    fn metadata(&self) -> &DatasetMetadata;

    /// Every work this dataset holds, in path order.
    fn works(&self) -> Result<Vec<WorkId>, DatasetError>;

    /// Every expression of one work, oldest first, without their trees.
    ///
    /// A work with one expression is ordinary, not degenerate: a court opinion
    /// is published once and never amended.
    fn expressions(&self, work: &WorkId) -> Result<Vec<ExpressionInfo>, DatasetError>;

    /// One work as it read on one date.
    ///
    /// `None` means this dataset holds no such expression. Ask [`works`] or the
    /// scope whether it holds the work at all, so a missing date is not
    /// mistaken for a missing document.
    ///
    /// [`works`]: DocumentReader::works
    fn get_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError>;

    /// The next expression of the same work, or `None` at the latest one.
    fn next_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError>;

    /// The previous expression of the same work, or `None` at the earliest one.
    fn prev_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError>;

    /// The differences between two expressions of one work.
    ///
    /// Both ids must name the same work. A diff across two works compares
    /// unrelated documents, so it is refused rather than answered.
    fn compute_diff(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError>;

    /// Search text across every expression.
    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError>;

    /// Find an element by path, in every expression that holds it.
    fn find_element(&self, path: &str) -> Result<Vec<(ExpressionId, USLMElement)>, DatasetError>;

    /// Whether any expression holds at least one provision at `path`.
    ///
    /// A path locates provisions, it does not identify one
    /// (`docs/adr/0001-structural-paths-locate-not-identify.md`), so the
    /// question is "at least one", never "exactly one". Ask this rather than
    /// [`find_element`] wherever the element itself is not wanted: a backend
    /// can answer it from an index, without reading a document.
    ///
    /// [`find_element`]: DocumentReader::find_element
    fn has_element(&self, path: &str) -> Result<bool, DatasetError>;
}

/// Reading the links a dataset holds.
///
/// A link connects a provision to something else and carries its provenance.
/// Links are core, not part of any extension, so that a reader which does not
/// know an extension can still see, report, and preserve its links
/// (`docs/adr/0002-links-live-in-the-core.md`).
///
/// Today the only kind is the change annotation. `annotations_for_bill` is a
/// query by link object, and the bill id is an opaque string here: this trait
/// does not need to know what a bill is.
pub trait LinkReader {
    /// Every link whose subject names this structural path.
    fn links_for_path(&self, path: &str) -> Result<Vec<Link>, DatasetError>;

    /// Every link about a change between two expressions of one work.
    fn links_for_pair(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Vec<Link>, DatasetError>;

    /// Every link of one kind, such as `legislature.amended_by`.
    fn links_by_kind(&self, kind: &str) -> Result<Vec<Link>, DatasetError>;

    /// Every link in one namespace, understood or not.
    ///
    /// This is the query a reader uses to report links it cannot interpret,
    /// which is the whole point of the kind being an open string
    /// (`docs/adr/0002-links-live-in-the-core.md`).
    fn links_by_namespace(&self, namespace: &str) -> Result<Vec<Link>, DatasetError>;

    /// Every link whose object reference starts with this prefix.
    ///
    /// An amendment reference is `legislature.amendment:<bill>:<amendment>`, so
    /// a bill's links are a prefix query.
    fn links_for_object_prefix(&self, prefix: &str) -> Result<Vec<Link>, DatasetError>;

    /// Every expression pair that carries links.
    fn link_pairs(&self) -> Result<Vec<crate::dataset::ExpressionPair>, DatasetError>;

    /// How many links are held of each kind, keyed by the kind named in full.
    ///
    /// A count, not a load: a backend that can count answers without building
    /// the links. Broken down by kind because a total folds a namespace this
    /// build has never seen into a number that hides it, and naming an unknown
    /// kind is exactly what a reader can still do with it
    /// (`docs/adr/0002-links-live-in-the-core.md`).
    fn count_links_by_kind(&self) -> Result<BTreeMap<String, usize>, DatasetError>;

    // --- Projections ---
    //
    // `ChangeAnnotation` is a view of links, the reverse of how it once was.
    // These are implemented once here rather than per backend: they are the
    // same regrouping whatever holds the links.

    /// Annotations recorded for a pair of expressions of one work.
    fn get_annotations(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        let links = self.links_for_pair(from, to)?;
        if links.is_empty() {
            return Ok(None);
        }
        Ok(Some(crate::link::annotations_from_links(&links)))
    }

    /// Every annotation naming this path, across all pairs.
    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        Ok(crate::link::annotations_from_links(
            &self.links_for_path(path)?,
        ))
    }

    /// Every annotation from one bill, across all pairs.
    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        let prefix = format!("legislature.amendment:{bill_id}:");
        Ok(crate::link::annotations_from_links(
            &self.links_for_object_prefix(&prefix)?,
        ))
    }

    /// Every expression pair that carries annotations.
    fn annotation_pairs(&self) -> Result<Vec<crate::dataset::ExpressionPair>, DatasetError> {
        self.link_pairs()
    }
}

/// Reading the legislature facts a dataset holds.
///
/// An extension, not core. Implement it only for a backend that actually holds
/// bills. A dataset of court opinions does not, and a caller reading a file
/// from another party must be able to find that out rather than get an error
/// from a method that should never have existed for that data.
pub trait LegislatureReader {
    /// Get a bill by ID
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError>;

    /// List the IDs of every bill in the dataset
    fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError>;

    /// Get a Congress member by bioguide ID
    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError>;

    /// Get sponsor info for a bill
    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError>;

    /// Get votes for a bill
    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError>;

    /// Get all roll calls where a member voted, with their position
    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError>;

    /// How much legislative material is held, counted rather than loaded.
    ///
    /// One method for the five counts, because a caller asking what a dataset
    /// holds wants all of them and a backend answers each with a count query.
    fn legislature_counts(&self) -> Result<LegislatureCounts, DatasetError>;
}

/// How much legislative material a dataset holds.
///
/// Counts only. A reader deciding whether a file is worth opening needs the
/// sizes, not the records, and the records are large.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LegislatureCounts {
    pub bills: usize,
    pub members: usize,
    /// Sponsor records, which is one per bill, not one per person.
    pub sponsors: usize,
    pub roll_calls: usize,
    /// One member's position in one roll call, summed over every roll call.
    pub member_votes: usize,
}

/// Reading the evidence a dataset holds.
///
/// Core, not an extension. A reader that does not know the legislature must
/// still be able to see what a machine's claim was based on, or the claim's
/// verification state is a label rather than something checkable (#58).
pub trait EvidenceReader {
    /// One verbatim model reply, by its id, or `None` when unheld.
    ///
    /// `None` is an answer: a dataset that never recorded a reply is a
    /// different thing from one whose reply was empty.
    fn get_reply(&self, id: &str) -> Result<Option<String>, DatasetError>;

    /// Every reply id this dataset holds, in id order.
    fn replies(&self) -> Result<Vec<String>, DatasetError>;

    /// How many replies this dataset holds.
    ///
    /// A count, not a load. Listing the ids to count them costs one string per
    /// reply, and a real sweep holds thousands.
    fn count_replies(&self) -> Result<usize, DatasetError>;
}

/// Writing evidence.
pub trait EvidenceWriter {
    /// Record a verbatim model reply and return its id.
    ///
    /// A reply is identified by the hash of its own text, so recording the same
    /// reply twice leaves one record. Evidence is append-only: a reply whose
    /// statement was later superseded is kept, because deleting it destroys the
    /// trail it exists to create.
    fn add_reply(&mut self, reply: &str) -> Result<String, DatasetError>;
}

/// Writing documents and dataset metadata.
///
/// Reading metadata lives on [`DocumentReader`], not here.
pub trait DocumentWriter {
    /// Set metadata
    fn set_metadata(&mut self, metadata: DatasetMetadata);

    /// Add one expression of one work.
    ///
    /// Adding the same `(work, date)` twice replaces the earlier one, so a
    /// rebuild cannot leave two trees claiming to be the same text.
    fn add_expression(&mut self, expression: Expression) -> Result<(), DatasetError>;
}

/// Writing links.
pub trait LinkWriter {
    /// Record one link.
    ///
    /// A link is identified by what it says, so writing the same subject, kind,
    /// and object twice leaves one link. When the stored link has been touched
    /// by a human — `HumanConfirmed`, `Disputed`, or `Refuted` — its provenance
    /// survives, and the incoming one is dropped. Otherwise the new provenance
    /// replaces the old. Without that rule, re-running the pipeline destroys
    /// human review quietly
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    ///
    /// There is deliberately no annotation-shaped convenience beside this. Two
    /// ways to write one fact means the convenient one is used, and the
    /// convenient one can only express the single kind we own.
    fn add_link(&mut self, link: Link) -> Result<(), DatasetError>;
}

/// Writing legislature facts. An extension, like [`LegislatureReader`].
pub trait LegislatureWriter {
    /// Add a bill
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError>;

    /// Add a Congress member
    fn add_member(&mut self, member: Member) -> Result<(), DatasetError>;

    /// Add sponsor info
    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError>;

    /// Add bill votes
    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError>;
}

/// The full set of capabilities, which both first-party backends provide.
///
/// Code that needs everything binds to this. Code that needs only documents
/// should bind to [`DocumentReader`] instead, so it keeps working against a
/// dataset that carries no legislative material.
pub trait Storage:
    DocumentReader
    + LinkReader
    + EvidenceReader
    + LegislatureReader
    + DocumentWriter
    + LinkWriter
    + EvidenceWriter
    + LegislatureWriter
{
    /// The legislature extension, when this dataset actually carries one.
    ///
    /// This is the run-time door. A dataset arriving from another party is
    /// whatever it is, and the caller cannot know at compile time whether it
    /// holds bills. `None` means "this dataset has no legislative material",
    /// which is a different answer from "it has no bills matching your query",
    /// and the difference is the one a researcher needs.
    ///
    /// Code of our own that always needs bills should take
    /// `impl DocumentReader + LegislatureReader` instead, and let the compiler
    /// enforce it.
    ///
    /// A dataset that declares the `legislature` namespace holds a legislature,
    /// whatever its contents. Deciding from contents alone reported that a
    /// legislative dataset held no legislature until the first bill arrived,
    /// which is absence mistaken for intent. A dataset that declares nothing
    /// still answers from its contents, because that is every dataset written
    /// before declarations existed.
    fn legislature(&self) -> Option<&dyn LegislatureReader>;
}
