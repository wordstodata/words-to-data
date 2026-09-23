//! Dataset module for versioned US Code data with bill annotations
//!
//! This module provides the `Dataset` struct for storing and exploring
//! versioned legal documents, designed for the SLEUTH Tauri app.

mod error;
mod scope;
mod work;

pub use error::DatasetError;
pub use scope::{Coverage, DateRange, Declaration, Exclusion, Scope, WorkCoverage};
pub use work::{
    Expression, ExpressionId, ExpressionInfo, ParseExpressionIdError, WorkId, WorksBetween,
    adjacent_expressions, bill_document, work_roots, works_between,
};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::annotation::ChangeAnnotation;
use crate::congress::{
    BillDownload, BillVotes, CosponsorRecord, HouseRollCall, Member, SponsorInfo, VotePosition,
};
use crate::diff::{Redesignations, TreeDiff};
use crate::document::DocumentNode;
use crate::legislature::BillDiff;
use crate::legislature::redesignation::{self, RedesignationReport};
use crate::link::{Link, ProvisionHistory};
use crate::storage::{
    DocumentReader, DocumentWriter, EvidenceReader, EvidenceWriter, InMemoryStorage,
    LegislatureCounts, LegislatureReader, LegislatureWriter, LinkReader, LinkWriter, SqliteStorage,
    Storage,
};
use crate::uslm::bill_parser::Bill;
use crate::uslm::parser::ParseError;
use crate::utils::{load_uslm_folder, parse_uslm_xml};

/// On-disk serialization format for in-memory datasets.
///
/// SQLite is a backend, not a serialization format. To work with SQLite-backed
/// datasets, use [`Dataset::open_sqlite`] / [`Dataset::new_sqlite`] (lazy access)
/// or [`Dataset::save_to_sqlite`] (dump in-memory data to a SQLite file).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    /// Compact format with deduplicated string table (smaller, faster)
    #[default]
    Compact,
    /// Raw JSON with full data (larger, for debugging/interop)
    Json,
}

/// Metadata describing a dataset
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
    pub source_urls: Vec<String>,
    pub license: String,
    pub version: String,
    /// What this dataset was meant to carry, when the producer said.
    ///
    /// `None` means nothing was declared, and every scope question is answered
    /// from the contents alone.
    #[serde(default)]
    pub declaration: Option<Declaration>,
}

/// A search result from text search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The expression the hit was found in.
    pub expression: ExpressionId,
    pub path: String,
    pub field: String,
    pub snippet: String,
}

/// A source this build could not read, as a dataset error.
///
/// One spelling for the two readers of a bill's markup here. `DatasetError`
/// carries no parse variant, and writing the same four lines of wrapping at each
/// call site is how one of them ends up saying something different from the
/// other.
fn invalid_data(cause: &dyn std::fmt::Display) -> DatasetError {
    DatasetError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        cause.to_string(),
    ))
}

/// The two expressions an annotation sits between. Both name the same work.
pub type ExpressionPair = (ExpressionId, ExpressionId);

/// A collection of versioned legal documents with bill annotations
///
/// Generic over storage backend. Use `Dataset<InMemoryStorage>` for in-memory
/// or `Dataset<SqliteStorage>` for database-backed storage.
pub struct Dataset<S: Storage> {
    storage: S,
}

impl<S: Storage> Dataset<S> {
    /// Create a dataset with the given storage backend
    pub fn with_storage(storage: S) -> Self {
        Self { storage }
    }

    /// Get reference to underlying storage
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Get mutable reference to underlying storage
    pub fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }

    /// What this dataset covers.
    ///
    /// Use it to tell "the law does not contain this" from "this dataset never
    /// held it":
    ///
    /// ```ignore
    /// if dataset.search_text(query)?.is_empty()
    ///     && dataset.scope()?.covers(path) == Coverage::OutOfScope
    /// {
    ///     // say "out of scope", not "not found"
    /// }
    /// ```
    pub fn scope(&self) -> Result<Scope, DatasetError> {
        Scope::derive(&self.storage)
    }

    /// Every work this dataset holds.
    ///
    /// A work is a document as a concept, with no date. This is the entry point
    /// for a dataset whose contents do not share a release cycle: ten court
    /// opinions are ten works, and no global date describes them.
    pub fn works(&self) -> Result<Vec<WorkId>, DatasetError> {
        self.storage.works()
    }

    /// Every expression of one work, in date order.
    ///
    /// A work with one expression is normal, not a degenerate case: a court
    /// opinion is published once and never amended.
    pub fn expressions(&self, work: &WorkId) -> Result<Vec<ExpressionInfo>, DatasetError> {
        self.storage.expressions(work)
    }

    /// One work as it read on one date.
    ///
    /// `None` means this dataset holds no such expression. Ask
    /// [`Dataset::scope`] whether it holds the work at all, so an absent
    /// expression is not mistaken for an absent document.
    pub fn get_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.storage.get_expression(id)
    }

    /// The next expression of the same work, or `None` at the latest one.
    pub fn next_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.storage.next_expression(id)
    }

    /// The previous expression of the same work, or `None` at the earliest one.
    pub fn prev_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.storage.prev_expression(id)
    }

    /// The legislature extension, when this dataset carries one.
    ///
    /// Use this when reading a dataset whose contents you do not control:
    ///
    /// ```ignore
    /// match dataset.legislature() {
    ///     Some(legislature) => legislature.get_bill(id)?,
    ///     None => return Err("this dataset holds no legislative material".into()),
    /// }
    /// ```
    ///
    /// `None` means the dataset has no legislative material at all, which is a
    /// different answer from "no bill matches that id".
    pub fn legislature(&self) -> Option<&dyn LegislatureReader> {
        self.storage.legislature()
    }

    // --- Delegate reader methods ---

    pub fn get_annotations(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        self.storage.get_annotations(from, to)
    }

    /// The differences between two expressions of one work.
    ///
    /// Where a redesignation link says a provision was renumbered between these
    /// two dates, the provision is paired with what it became, so the provision
    /// it displaced reads as removed rather than as rewritten. Where none is
    /// known, pairing is by position, exactly as before (#93).
    ///
    /// [`DocumentReader::compute_diff`] on a bare backend cannot do this, because
    /// a document reader holds no links. A dataset holds both.
    pub fn compute_diff(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError> {
        self.diff_over_links(from, to)
    }

    /// The diff, with a renumbered provision paired to what it became.
    ///
    /// Delegates to the backend when this dataset holds no redesignation for the
    /// pair, which is every dataset until `record_redesignations` has run. That
    /// keeps the ordinary path exactly as fast as it was, and keeps a backend
    /// free to answer a diff its own way.
    fn diff_over_links(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError> {
        let known = Redesignations::from_links(&self.storage.links_for_pair(from, to)?);
        if known.is_empty() {
            return self.storage.compute_diff(from, to);
        }
        let (from_expression, to_expression) =
            crate::storage::memory::require_same_work(&self.storage, from, to)?;
        Ok(TreeDiff::from_nodes_with(
            &from_expression.root,
            &to_expression.root,
            &known,
        ))
    }

    /// Read the redesignations a bill states and record each one as a link.
    ///
    /// `from` must be the earlier expression and `to` the later one: a
    /// redesignation moves a provision *away* from one path and *to* another, and
    /// each path exists on one side of the move only, so a statement is checked
    /// against both documents.
    ///
    /// Takes statements the caller has already read, rather than the bill this
    /// dataset stores. Which provision a clause is about comes from where the
    /// words sat in the bill's markup — a clause inside "in subsection (a)--"
    /// means something different from the same clause outside it — and a stored
    /// amendment keeps only the flattened text. Read them with
    /// [`crate::uslm::bill_redesignation::redesignations_stated`].
    ///
    /// Returns the report, including every statement it could not place. A
    /// caller that drops the report turns this build's silence into the corpus's
    /// silence.
    pub fn record_redesignations(
        &mut self,
        bill_id: &str,
        stated: &[redesignation::StatedRedesignation],
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<RedesignationReport, DatasetError> {
        let (from_expression, to_expression) =
            crate::storage::memory::require_same_work(&self.storage, from, to)?;
        let report = redesignation::resolve(stated, &from_expression.root, &to_expression.root);
        for resolved in &report.resolved {
            self.storage
                .add_link(resolved.link(&from.work, &from.at, &to.at, bill_id))?;
        }
        Ok(report)
    }

    /// Record every redesignation a bill states, over the windows named.
    ///
    /// **The explicit step (#181).** Loading a bill records nothing, because at
    /// load time nobody knows which window matters and often the window is not
    /// held yet. Whoever knows names the windows here: an operator gives them
    /// with `--between` or `--from`/`--to`, and `build-dataset` gives every
    /// window it holds after it has loaded everything. #172 decides which
    /// windows a bill may be tried against, and it changes the list a caller
    /// brings rather than this method.
    ///
    /// Takes the bill's own document, not its XML. Which provision a clause is
    /// about comes from where the words sat in the bill, and the stored document
    /// holds that nesting — which is the whole of what the second parse used to
    /// recover (ADR 0009). Read it with [`Dataset::bill_document`]. A statement
    /// resolves in the one work that holds its section and fails in every
    /// other, so the reports are folded rather than concatenated.
    ///
    /// Returns the report, including every statement it could not place. A
    /// caller that drops the report turns this build's silence into the
    /// corpus's silence: print it with [`RedesignationReport::warn`].
    pub fn record_redesignations_over(
        &mut self,
        bill_id: &str,
        bill: &DocumentNode,
        windows: &[ExpressionPair],
    ) -> Result<RedesignationReport, DatasetError> {
        let stated = crate::uslm::bill_redesignation::redesignations_stated_in(bill_id, bill);
        // A bill that renumbers nothing is ordinary, and reading every window to
        // prove it would cost a section index per work for no statement.
        if stated.is_empty() {
            return Ok(RedesignationReport::default());
        }

        // A bill named against no window has nothing to be checked against.
        // Every statement it makes is unplaced, and saying nothing would read as
        // a bill that renumbered nothing (#153).
        if windows.is_empty() {
            return Ok(RedesignationReport::without_a_window(&stated));
        }

        let mut per_work = Vec::new();
        for (from, to) in windows {
            per_work.push(self.record_redesignations(bill_id, &stated, from, to)?);
        }
        Ok(RedesignationReport::across_works(per_work))
    }

    /// The renumberings one provision ran through, oldest first.
    ///
    /// A projection over the redesignation links, walked on demand. See
    /// [`LinkReader::provision_history`].
    pub fn provision_history(&self, path: &str) -> Result<ProvisionHistory, DatasetError> {
        self.storage.provision_history(path)
    }

    pub fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        self.storage.search_text(query)
    }

    pub fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        self.storage.annotations_for_path(path)
    }

    pub fn annotations_for_bill(
        &self,
        bill_id: &str,
    ) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        self.storage.annotations_for_bill(bill_id)
    }

    pub fn annotation_pairs(&self) -> Result<Vec<ExpressionPair>, DatasetError> {
        self.storage.annotation_pairs()
    }

    pub fn find_nodes(
        &self,
        path: &str,
    ) -> Result<Vec<(ExpressionId, DocumentNode)>, DatasetError> {
        self.storage.find_nodes(path)
    }

    // --- Delegate DatasetWriter methods ---

    pub fn metadata(&self) -> &DatasetMetadata {
        self.storage.metadata()
    }

    pub fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.storage.set_metadata(metadata)
    }

    pub fn add_expression(&mut self, expression: Expression) -> Result<(), DatasetError> {
        self.storage.add_expression(expression)
    }

    /// Record a verbatim model reply and return its id.
    pub fn add_reply(&mut self, reply: &str) -> Result<String, DatasetError> {
        self.storage.add_reply(reply)
    }

    /// One verbatim model reply, by its id.
    pub fn get_reply(&self, id: &str) -> Result<Option<String>, DatasetError> {
        self.storage.get_reply(id)
    }

    /// Record one link.
    ///
    /// There is no annotation-shaped convenience beside this: two ways to write
    /// one fact means the convenient one is used, and the convenient one can
    /// only ever express the single kind we own
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    pub fn add_link(&mut self, link: Link) -> Result<(), DatasetError> {
        self.storage.add_link(link)
    }

    /// Get paths that have annotations for an expression pair
    pub fn annotated_paths(&self, from: &ExpressionId, to: &ExpressionId) -> Vec<String> {
        self.get_annotations(from, to)
            .ok()
            .flatten()
            .map(|annotations| {
                annotations
                    .iter()
                    .flat_map(|a| a.paths.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get paths with changes that lack annotations for an expression pair
    pub fn unannotated_paths(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Vec<String>, DatasetError> {
        let diff = self.compute_diff(from, to)?;
        let annotated = self.annotated_paths(from, to);
        let annotated_set: std::collections::HashSet<_> = annotated.into_iter().collect();

        let mut paths_with_changes = Vec::new();
        Self::collect_paths_with_changes(&diff, &mut paths_with_changes);

        Ok(paths_with_changes
            .into_iter()
            .filter(|p| !annotated_set.contains(p))
            .collect())
    }

    fn collect_paths_with_changes(diff: &TreeDiff, paths: &mut Vec<String>) {
        if !diff.changes.is_empty() || !diff.added.is_empty() || !diff.removed.is_empty() {
            paths.push(diff.root_path.clone());
        }
        for child in &diff.child_diffs {
            Self::collect_paths_with_changes(child, paths);
        }
    }
}

// --- Legislature extension ---
//
// One impl block per extension trait rather than a `where` clause on each of
// the ten methods. The requirement is then stated once, in the place a reader
// looks for it, and the block itself says which methods exist only because the
// backend speaks the extension. Ten repetitions of the same clause says the
// same thing to the compiler and less to a person (#127).
//
// The bound is not on the struct: a dataset of court opinions is a dataset, and
// it keeps every core method above.

/// Reading the legislature, for a dataset whose backend holds one.
impl<S: Storage + LegislatureReader> Dataset<S> {
    pub fn get_bill(&self, bill_id: &str) -> Result<Option<Bill>, DatasetError> {
        self.storage.get_bill(bill_id)
    }

    /// One bill's own document, as this dataset holds it.
    ///
    /// A bill is a work like any other, stored under the number its publisher
    /// gave it — `publiclawdocument_119-21` — while the dataset knows the bill
    /// by the id it was downloaded under, `119-hr-1`. The two are joined by what
    /// the bill says rather than by a second name written down twice: every
    /// instruction in the stored document carries the content hash minted under
    /// the dataset's id, so the document that states this bill's amendments is
    /// this bill's document
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    ///
    /// `None` means this dataset holds no document for that bill. A dataset
    /// built before #196 holds the bill's amendments and no document, which is
    /// the same answer: there is nothing here to read.
    ///
    /// Only works whose path opens with a public law's are opened, because
    /// reading every title of the Code to find one bill would cost the whole
    /// corpus. The node type below is what says the class; the path is a filter.
    pub fn bill_document(&self, bill_id: &str) -> Result<Option<Expression>, DatasetError> {
        bill_document(&self.storage, &self.storage, bill_id)
    }

    /// List the IDs of every bill in the dataset.
    ///
    /// Pair with [`Dataset::get_bill`] to iterate over all bills:
    /// ```ignore
    /// for id in dataset.list_bill_ids()? {
    ///     let bill = dataset.get_bill(&id)?.unwrap();
    /// }
    /// ```
    pub fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError> {
        self.storage.list_bill_ids()
    }

    pub fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        self.storage.get_member(bioguide_id)
    }

    pub fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        self.storage.get_sponsor_info(bill_id)
    }

    pub fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        self.storage.get_bill_votes(bill_id)
    }

    pub fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        self.storage.votes_by_member(bioguide_id)
    }
}

/// Writing the legislature, for a dataset whose backend holds one.
impl<S: Storage + LegislatureWriter> Dataset<S> {
    pub fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        self.storage.add_bill(bill)
    }

    pub fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        self.storage.add_member(member)
    }

    pub fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        self.storage.add_sponsor_info(info)
    }

    pub fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        self.storage.add_bill_votes(votes)
    }
}

// --- Reading USLM into any backend ---
//
// A release point is parsed once and becomes one expression per work, and where
// those expressions land is the backend's business. These methods sat on the
// in-memory dataset alone, which is why a SQLite dataset could not grow (#180).
// Nothing stored changes by making them generic: an expression is still
// `(work, date)` (`docs/adr/0003-storage-is-keyed-by-work.md`).
impl<S: Storage> Dataset<S> {
    /// Parse a USLM XML file and add each work it holds as an expression.
    pub fn add_uslm_xml(
        &mut self,
        xml_path: &str,
        date: &str,
        label: Option<String>,
    ) -> Result<(), ParseError> {
        let element = parse_uslm_xml(xml_path, date)?;
        self.add_works_of(element, date, label)
            .map_err(|e| ParseError::Io(std::io::Error::other(e)))
    }

    /// Load and merge all USLM XML files from a folder, then add each work it
    /// holds as an expression.
    ///
    /// A US Code release point is one folder of many titles. It arrives as one
    /// merged tree and leaves as one expression per title, because the release
    /// point is how the source publishes, not what the dataset holds.
    pub fn add_uslm_folder(
        &mut self,
        folder_path: &str,
        date: &str,
        label: Option<String>,
    ) -> Result<(), DatasetError> {
        let element = load_uslm_folder(folder_path, date)
            .ok_or_else(|| DatasetError::FolderLoadFailed(folder_path.to_string()))?;
        self.add_works_of(element, date, label)
    }

    /// Split a parsed tree into works and add one expression for each.
    fn add_works_of(
        &mut self,
        parsed: DocumentNode,
        date: &str,
        label: Option<String>,
    ) -> Result<(), DatasetError> {
        for root in work_roots(parsed) {
            self.add_expression(Expression {
                id: ExpressionId::new(WorkId::new(root.data.path.to_string()), date),
                label: label.clone(),
                root,
            })?;
        }
        Ok(())
    }
}

// --- InMemoryStorage-specific methods ---

impl Clone for Dataset<InMemoryStorage> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
        }
    }
}

impl Dataset<InMemoryStorage> {
    /// Create a new in-memory dataset with the given metadata
    pub fn new(metadata: DatasetMetadata) -> Self {
        Self::with_storage(InMemoryStorage::new(metadata))
    }

    /// Add changes to an amendment in any bill
    pub fn add_changes_to_amendment(&mut self, amendment_id: &str, bill_diff: &BillDiff) {
        for bill in self.storage.bills.values_mut() {
            if let Some(amendment) = bill.amendments.get_mut(amendment_id) {
                amendment.changes.push(bill_diff.clone());
                return;
            }
        }
    }

    /// Record where an amendment's word-level changes came from.
    ///
    /// The amending text is parsed from the bill and is a fact from a source.
    /// The changes are a model's reading of it, and until this existed nothing
    /// said so (#58).
    pub fn set_amendment_provenance(
        &mut self,
        amendment_id: &str,
        provenance: crate::link::Provenance,
    ) {
        for bill in self.storage.bills.values_mut() {
            if let Some(amendment) = bill.amendments.get_mut(amendment_id) {
                amendment.provenance = Some(provenance);
                return;
            }
        }
    }

    /// Save to file in specified format
    pub fn save(&self, path: &str, format: Format) -> Result<(), DatasetError> {
        match format {
            Format::Compact => {
                use crate::compact::DatasetCompact;
                let compact = DatasetCompact::from_storage(self.storage());
                let json = serde_json::to_string(&compact)?;
                fs::write(path, json)?;
            }
            Format::Json => {
                let json = serde_json::to_string_pretty(self.storage())?;
                fs::write(path, json)?;
            }
        }
        Ok(())
    }

    /// Load from file in specified format
    ///
    /// A file written by a different schema is refused rather than half-read.
    /// Datasets are rebuilt, not migrated, so the break has to be loud.
    pub fn load(path: &str, format: Format) -> Result<Self, DatasetError> {
        match format {
            Format::Compact => {
                use crate::compact::{DatasetCompact, check_schema_version};
                // Before the full parse: every other field changes shape
                // between schemas, so parsing first would fail on one of those
                // and report a type mismatch instead of "rebuild this file".
                //
                // The check streams and stops at the version, so a file this
                // build cannot read is refused without being read. The file is
                // then opened a second time for the parse itself, which reads
                // the text in one go because that is the faster way to parse a
                // large file.
                check_schema_version(io::BufReader::new(fs::File::open(path)?))?;
                let json = fs::read_to_string(path)?;
                let compact: DatasetCompact = serde_json::from_str(&json)?;
                Ok(Self::with_storage(compact.into_storage()))
            }
            Format::Json => {
                let json = fs::read_to_string(path)?;
                let mut storage: InMemoryStorage = serde_json::from_str(&json)?;
                storage.intern_strings();
                Ok(Self::with_storage(storage))
            }
        }
    }

    /// Dump this in-memory dataset to a SQLite file at `path`.
    ///
    /// To then query the file lazily (without re-loading into memory),
    /// use [`Dataset::open_sqlite`].
    pub fn save_to_sqlite<P: AsRef<Path>>(&self, path: P) -> Result<(), DatasetError> {
        let mut sqlite = SqliteStorage::open(path)?;
        sqlite.save_from_memory(self.storage())?;
        Ok(())
    }

    /// Load bill data from a BillDownload
    pub fn load_bill_download(&mut self, download: &BillDownload) -> Result<String, DatasetError> {
        use crate::uslm::bill_parser;
        use serde_json::Value;

        // One read of the XML, for every reader of it. The bill's amendments and
        // the bill's document used to be two parses of the same 2.7 MB string
        // (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).
        let markup =
            roxmltree::Document::parse(&download.bill_xml).map_err(|e| invalid_data(&e))?;

        let (bill, amendment_report) = bill_parser::bill_of_document(&markup, &download.bill_id);
        amendment_report.print_to_stderr();
        let bill_id = bill.bill_id.clone();
        self.add_bill(bill)?;

        // The bill itself, with the structure the parser found. Nothing stored
        // it before this, so the nesting a redesignation is read out of existed
        // only inside one function call and was then thrown away.
        let (expression, parse_report) =
            bill_parser::bill_expression(&markup, &bill_id).map_err(|e| invalid_data(&e))?;
        parse_report.print_to_stderr();

        // No redesignation is recorded here. Loading a bill loads a bill: the
        // links it states are written by an explicit step over a named window
        // (`Dataset::record_redesignations_over`, #181). Recording them here
        // needed a window, nobody at load time knows which one, and often the
        // window is not held yet — so the step swept every neighbouring pair
        // the dataset happened to hold (#172) and still held for one build
        // order only (#180).
        self.add_expression(expression)?;

        // Parse sponsor from metadata
        let sponsors_v: Value = serde_json::from_str(&download.bill_metadata_json)?;
        let sponsor_id = sponsors_v["bill"]["sponsors"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["bioguideId"].as_str())
            .unwrap_or("")
            .to_string();

        // Parse cosponsors
        let cosponsors_v: Value = serde_json::from_str(&download.cosponsors_json)?;
        let mut cosponsors = Vec::new();
        if let Some(arr) = cosponsors_v["cosponsors"].as_array() {
            for c in arr {
                cosponsors.push(CosponsorRecord {
                    bioguide_id: c["bioguideId"].as_str().unwrap_or("").to_string(),
                    date: c["sponsorshipDate"].as_str().unwrap_or("").to_string(),
                    withdrawn: c["sponsorshipWithdrawnDate"].as_str().is_some(),
                });
            }
        }

        self.add_sponsor_info(SponsorInfo {
            bill_id: bill_id.clone(),
            sponsor: sponsor_id,
            cosponsors,
        })?;

        // Parse and add members
        for json in download.member_jsons.values() {
            if let Ok(member) = Member::from_api_response(json) {
                self.add_member(member)?;
            }
        }

        // Parse votes
        if let Some(ref votes_json) = download.votes_json
            && let Ok(roll_calls) = serde_json::from_str::<Vec<HouseRollCall>>(votes_json)
        {
            self.add_bill_votes(BillVotes {
                bill_id: bill_id.clone(),
                roll_calls,
            })?;
        }

        Ok(bill_id)
    }
}

// --- SqliteStorage-specific methods ---

impl Dataset<SqliteStorage> {
    /// Open a SQLite-backed dataset
    pub fn open_sqlite<P: AsRef<Path>>(path: P) -> Result<Self, DatasetError> {
        Ok(Self::with_storage(SqliteStorage::open(path)?))
    }

    /// Create a new SQLite-backed dataset in memory
    pub fn new_sqlite(metadata: DatasetMetadata) -> Result<Self, DatasetError> {
        Ok(Self::with_storage(SqliteStorage::new_with_metadata(
            metadata,
        )?))
    }

    /// Materialize the full SQLite dataset as an in-memory dataset.
    pub fn to_memory(&self) -> Result<Dataset<InMemoryStorage>, DatasetError> {
        Ok(Dataset::with_storage(self.storage.to_memory()?))
    }

    /// Load just the expression pair `[from, to]` (and any annotations between
    /// them) into an in-memory dataset, without materializing the rest of the
    /// database.
    pub fn load_window(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Dataset<InMemoryStorage>, DatasetError> {
        Ok(Dataset::with_storage(self.storage.load_window(from, to)?))
    }
}

// --- Implement traits for Dataset<S> ---

impl<S: Storage> DocumentReader for Dataset<S> {
    fn metadata(&self) -> &DatasetMetadata {
        self.storage.metadata()
    }

    fn works(&self) -> Result<Vec<WorkId>, DatasetError> {
        self.storage.works()
    }

    fn expressions(&self, work: &WorkId) -> Result<Vec<ExpressionInfo>, DatasetError> {
        self.storage.expressions(work)
    }

    fn get_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.storage.get_expression(id)
    }

    fn next_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.storage.next_expression(id)
    }

    fn prev_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.storage.prev_expression(id)
    }

    /// The differences between two expressions, over the links as well as the
    /// documents.
    ///
    /// A dataset holds both, so it answers the better question. See
    /// [`Dataset::compute_diff`].
    fn compute_diff(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError> {
        self.diff_over_links(from, to)
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        self.storage.search_text(query)
    }

    fn find_nodes(&self, path: &str) -> Result<Vec<(ExpressionId, DocumentNode)>, DatasetError> {
        self.storage.find_nodes(path)
    }

    fn has_node(&self, path: &str) -> Result<bool, DatasetError> {
        self.storage.has_node(path)
    }
}

impl<S: Storage> LinkReader for Dataset<S> {
    fn links_for_path(&self, path: &str) -> Result<Vec<Link>, DatasetError> {
        self.storage.links_for_path(path)
    }

    fn links_for_pair(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Vec<Link>, DatasetError> {
        self.storage.links_for_pair(from, to)
    }

    fn links_by_kind(&self, kind: &str) -> Result<Vec<Link>, DatasetError> {
        self.storage.links_by_kind(kind)
    }

    fn links_by_namespace(&self, namespace: &str) -> Result<Vec<Link>, DatasetError> {
        self.storage.links_by_namespace(namespace)
    }

    fn links_for_object_prefix(&self, prefix: &str) -> Result<Vec<Link>, DatasetError> {
        self.storage.links_for_object_prefix(prefix)
    }

    fn link_pairs(&self) -> Result<Vec<ExpressionPair>, DatasetError> {
        self.storage.link_pairs()
    }

    fn count_links_by_kind(&self) -> Result<BTreeMap<String, usize>, DatasetError> {
        self.storage.count_links_by_kind()
    }
}

impl<S: Storage> EvidenceReader for Dataset<S> {
    fn get_reply(&self, id: &str) -> Result<Option<String>, DatasetError> {
        self.storage.get_reply(id)
    }

    fn replies(&self) -> Result<Vec<String>, DatasetError> {
        self.storage.replies()
    }

    fn count_replies(&self) -> Result<usize, DatasetError> {
        self.storage.count_replies()
    }
}

impl<S: Storage> EvidenceWriter for Dataset<S> {
    fn add_reply(&mut self, reply: &str) -> Result<String, DatasetError> {
        self.storage.add_reply(reply)
    }
}

impl<S: Storage + LegislatureReader> LegislatureReader for Dataset<S> {
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError> {
        self.storage.get_bill(id)
    }

    fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError> {
        self.storage.list_bill_ids()
    }

    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        self.storage.get_member(bioguide_id)
    }

    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        self.storage.get_sponsor_info(bill_id)
    }

    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        self.storage.get_bill_votes(bill_id)
    }

    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        self.storage.votes_by_member(bioguide_id)
    }

    fn legislature_counts(&self) -> Result<LegislatureCounts, DatasetError> {
        self.storage.legislature_counts()
    }
}

impl<S: Storage> DocumentWriter for Dataset<S> {
    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.storage.set_metadata(metadata)
    }

    fn add_expression(&mut self, expression: Expression) -> Result<(), DatasetError> {
        self.storage.add_expression(expression)
    }
}

impl<S: Storage> LinkWriter for Dataset<S> {
    fn add_link(&mut self, link: Link) -> Result<(), DatasetError> {
        self.storage.add_link(link)
    }
}

impl<S: Storage + LegislatureWriter> LegislatureWriter for Dataset<S> {
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        self.storage.add_bill(bill)
    }

    fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        self.storage.add_member(member)
    }

    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        self.storage.add_sponsor_info(info)
    }

    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        self.storage.add_bill_votes(votes)
    }
}

impl<S: Storage> Storage for Dataset<S> {
    fn legislature(&self) -> Option<&dyn LegislatureReader> {
        self.storage.legislature()
    }
}
