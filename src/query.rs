//! One vocabulary for asking a dataset what it holds.
//!
//! The filters are a **product**, not a choice. Before this, annotations were
//! asked for by a sum — an expression pair, **or** a bill, **or** a path — so
//! *"what did this bill change at § 174 in this window"* could not be expressed,
//! and the way through was to diff a whole title and grep the output (#234).
//!
//! The sum was not buying an index. Two of its three variants already read every
//! annotation the dataset held and filtered them in Rust, so widening it to a
//! conjunction costs nothing: the same pass with more predicates.
//!
//! **A query is class-neutral.** It names an object by a prefix of that object's
//! reference rather than by a field called `bill`, so a `westlaw.` or `judicial.`
//! reference filters with no change here
//! (`docs/adr/0002-links-live-in-the-core.md`). The CLI turns `--bill 119-hr-1`
//! into the prefix; this module does not know what a bill is.
//!
//! **A query naming nothing matches everything**, rather than being refused. An
//! answer carries the total beside the rows, so a run that returned only some of
//! them can say how many it left out. A reader that truncates in silence reads
//! as a complete one, which is the defect #220 and #227 were about.

use crate::link::{Link, Window};

/// How many rows a listing prints before it stops.
///
/// One constant, so two commands cannot disagree about what a screenful is.
/// `redesignation-report` chose 20 first and this follows it. A run that stops
/// says how many it did not show, because a silent cap reads as the whole answer.
pub const DEFAULT_LIMIT: usize = 20;

/// Where in a dataset to look: a work, a window, a path, or any combination.
///
/// Separate from [`LinkQuery`] because it is not about links. Full-text search
/// and a diff both need somewhere to look and neither reads a link, so the part
/// that names a place is factored out for them to share.
///
/// **Not a `Scope`.** `CONTEXT.md` already defines a Scope as what a dataset
/// *covers* — what it holds and what its producer declared it would — which is a
/// statement about the dataset. This is a statement about one question. The word
/// here follows `docs/adr/0001-structural-paths-locate-not-identify.md`: a path
/// locates, so the thing built out of paths, works and windows is a locator.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Locator {
    /// One work, such as `uscode/title_26`. `None` means every work.
    pub work: Option<String>,
    /// One window, as two dates. `None` means every window.
    ///
    /// A [`Window`] rather than the CLI's `Span`: `Span` is a `clap` type that
    /// resolves dates and belongs to the binary, and a library vocabulary must
    /// not reach into it.
    pub window: Option<Window>,
    /// One structural path. `None` means every path.
    pub path: Option<String>,
    /// Whether [`Self::path`] names the path alone or its subtree.
    pub matching: PathMatch,
}

impl Locator {
    /// Everywhere: every work, every window, every path.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn in_work(mut self, work: impl Into<String>) -> Self {
        self.work = Some(work.into());
        self
    }

    pub fn in_window(mut self, from_date: impl Into<String>, to_date: impl Into<String>) -> Self {
        self.window = Some(Window {
            from_date: from_date.into(),
            to_date: to_date.into(),
        });
        self
    }

    pub fn at_path(mut self, path: impl Into<String>, matching: PathMatch) -> Self {
        self.path = Some(path.into());
        self.matching = matching;
        self
    }

    /// Whether a link's subject sits inside this scope.
    fn holds(&self, link: &Link) -> bool {
        if let Some(work) = &self.work {
            // A change and an expression name their work; a node carries it in
            // the first segments of its path instead, so both are checked rather
            // than teaching this module to parse a path.
            let named = link.subject.work() == Some(work.as_str());
            let by_path = link
                .subject
                .path()
                .is_some_and(|path| path.starts_with(&format!("{work}/")));
            if !named && !by_path {
                return false;
            }
        }
        if let Some(window) = &self.window
            && link.subject.window().as_ref() != Some(window)
        {
            return false;
        }
        if let Some(path) = &self.path {
            let Some(named) = link.subject.path() else {
                return false;
            };
            if !self.matching.accepts(path, named) {
                return false;
            }
        }
        true
    }
}

/// Whether a path names only itself or everything beneath it too.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PathMatch {
    /// The path given and every path beneath it. The default, because it is
    /// what naming a section means.
    #[default]
    Subtree,
    /// Only the path given, exactly.
    Exact,
}

impl PathMatch {
    /// Whether something recorded at `found` answers for `asked`.
    pub(crate) fn accepts(self, asked: &str, found: &str) -> bool {
        match self {
            // Segment-aware, so `section_16` does not answer for `section_163`.
            Self::Subtree => crate::uslm::path::covers_path(asked, found),
            Self::Exact => asked == found,
        }
    }
}

/// A link's review state: the three verdicts, or nobody has said anything.
///
/// Four values, mirroring decision 21 of #179 exactly — `review.confirmed`,
/// `review.refuted`, `review.disputed`, and their absence. No coarser word like
/// *settled* is defined here: it is a fold over these, and two vocabularies for
/// one idea is how the filters came to disagree in the first place.
///
/// Derived, never stored. The newest review of a link is the one a reader
/// reports, so this is the verdict of that record
/// (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    /// No review names this link.
    Unreviewed,
    /// The newest review confirms it.
    Confirmed,
    /// The newest review refutes it.
    Refuted,
    /// The newest review disputes it: contested, and not settled.
    Disputed,
}

impl ReviewStatus {
    /// The status a winning verdict gives.
    pub fn of_verdict(verdict: crate::review::Verdict) -> Self {
        match verdict {
            crate::review::Verdict::Confirmed => Self::Confirmed,
            crate::review::Verdict::Refuted => Self::Refuted,
            crate::review::Verdict::Disputed => Self::Disputed,
        }
    }
}

/// Which links to return, as a conjunction of filters.
///
/// Every field is optional and they are all applied. A query built and never
/// narrowed matches every link the dataset holds.
///
/// **Which filters are cheap.** `kind`, `namespace`, `object_prefix` and a
/// locator's work, window and path are all promoted to indexed columns on the
/// links table (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
/// `status` is **not**, and cannot be: a link's review state is derived from the
/// reviews pointing at it, so filtering on it costs a pass over the matches
/// (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
/// It is named here rather than left out, because *"unreviewed amendment links
/// for this bill"* is the question a reviewer actually asks, and dropping one
/// term of it leaves the question unaskable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkQuery {
    /// Where to look.
    pub locator: Locator,
    /// One kind in full, such as `legislature.amended_by`.
    ///
    /// Matched literally, so a kind this build has never seen still filters. A
    /// closed list here would refuse `westlaw.headnote`, which is the whole
    /// point of a kind being an open string (`docs/adr/0002`).
    pub kind: Option<String>,
    /// One namespace, such as `review`, which is every kind inside it.
    ///
    /// A namespace is a prefix of a kind, so one index serves both.
    pub namespace: Option<String>,
    /// A prefix of the object's reference, such as
    /// `legislature.amendment:119-hr-1:` for one bill's amendments.
    pub object_prefix: Option<String>,
    /// The review state a link must be in.
    ///
    /// The one filter with no index behind it: see this type's own note.
    pub status: Option<ReviewStatus>,
    /// How many rows to return. `None` returns all of them.
    ///
    /// The total is counted either way, so a truncated answer can say what it
    /// left out.
    pub limit: Option<usize>,
}

impl LinkQuery {
    /// Every link the dataset holds.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn at(mut self, locator: Locator) -> Self {
        self.locator = locator;
        self
    }

    pub fn of_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn in_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn with_object_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.object_prefix = Some(prefix.into());
        self
    }

    pub fn with_status(mut self, status: ReviewStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Whether this link answers the query.
    pub(crate) fn accepts(&self, link: &Link) -> bool {
        if let Some(kind) = &self.kind
            && link.kind.0 != *kind
        {
            return false;
        }
        if let Some(namespace) = &self.namespace
            && link.kind.namespace() != namespace
        {
            return false;
        }
        if let Some(prefix) = &self.object_prefix
            && !link.object.name().starts_with(prefix.as_str())
        {
            return false;
        }
        self.locator.holds(link)
    }
}

/// What a query returned, and how much it matched.
///
/// The total is the number of matches, not the number of rows: a limited query
/// returns fewer rows and the same total, which is what lets a caller say how
/// many it did not show. Reporting only the rows is how a truncated answer comes
/// to read as a complete one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer<T> {
    /// The rows returned, at most the query's limit.
    pub rows: Vec<T>,
    /// How many matched, whether or not they were returned.
    pub total: usize,
}

impl<T> Answer<T> {
    /// How many matches were not returned. Zero when nothing was truncated.
    pub fn dropped(&self) -> usize {
        self.total.saturating_sub(self.rows.len())
    }
}
