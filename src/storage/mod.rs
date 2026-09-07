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

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, HouseRollCall, Member, SponsorInfo, VotePosition};
use crate::dataset::{
    DatasetError, DatasetMetadata, Expression, ExpressionId, ExpressionInfo, SearchResult, WorkId,
};
use crate::diff::TreeDiff;
use crate::uslm::USLMElement;
use crate::uslm::bill_parser::Bill;

/// The dataset shape this build reads and writes.
///
/// Datasets are rebuilt rather than migrated, so a schema change is a clean
/// break. Both on-disk forms carry this number and refuse a file that does not
/// match, because a break that is not loud reads as an empty dataset.
///
/// 3 is the work-scoped schema: an expression is `(work, date)` with its own
/// tree, and annotations are keyed by a pair of expressions
/// (`docs/adr/0003-storage-is-keyed-by-work.md`).
///
/// One number covering both forms cannot describe a change to only one of
/// them. Widening the SQLite search index (#82) left the compact JSON
/// untouched, because its element trees always carried every text field, so
/// bumping this would have rejected valid JSON datasets to fix a SQLite table.
/// That case is caught where it happens, when the database is opened, rather
/// than here. A change that alters both forms still belongs to this number.
pub const SCHEMA_VERSION: i32 = 4;

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
    /// Get annotations recorded for a pair of expressions of one work.
    fn get_annotations(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError>;

    /// Find all annotations that include the given path (across all pairs)
    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError>;

    /// Find all annotations from a specific bill (across all pairs)
    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError>;

    /// List every expression pair that carries annotations
    fn annotation_pairs(&self) -> Result<Vec<crate::dataset::ExpressionPair>, DatasetError>;
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
    /// Add an annotation for a pair of expressions of one work.
    fn add_annotation(
        &mut self,
        from: &ExpressionId,
        to: &ExpressionId,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError>;
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
    DocumentReader + LinkReader + LegislatureReader + DocumentWriter + LinkWriter + LegislatureWriter
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
