//! What a dataset covers.
//!
//! A dataset is a ball of legal information of any size: one title, or all of
//! federal and state law. So a query that finds nothing is ambiguous. Did the
//! tool find no section 174, or does this dataset not hold title 26 at all? For
//! legal research that difference decides cases, because a reader who takes
//! absence for "no such authority" has been misled by the tool.
//!
//! [`Scope`] removes the ambiguity. It reports what the dataset holds, so a
//! caller can answer "out of scope" instead of "not found".
//!
//! It reports each work with its own dates, because a dataset need not hold
//! every work on every date. Two flat lists would say a dataset holding title 9
//! in July and title 51 in August holds both in both, which is a confident
//! wrong answer of exactly the kind this type exists to prevent.
//!
//! A scope has two halves. The **held** half is derived from the contents. The
//! **declared** half is what a producer says the dataset is meant to carry, and
//! the difference between them is a gap: material that was intended, is not
//! excluded, and is not here. A gap means the build did not do what it said.
//!
//! A hole the producer states, with its reason, is an [`Exclusion`] rather than
//! a gap. "Title 26 except section 174, because the source failed" is a
//! complete answer; an unexplained absence is not.

use serde::{Deserialize, Serialize};

use crate::dataset::{DatasetError, WorkId};
use crate::storage::DocumentReader;

/// Whether a dataset covers something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    /// The dataset holds this material. An empty result means the thing is
    /// genuinely absent from the law as this dataset records it.
    InScope,
    /// The dataset does not hold this material. An empty result says nothing
    /// about the law: we simply never had the text.
    OutOfScope,
    /// The dataset was declared to hold this and does not.
    ///
    /// Distinct from `OutOfScope` on purpose: the dataset is incomplete rather
    /// than out of its lane, so an empty result is a fault in the build and not
    /// a statement about the law or about what this dataset set out to do.
    Gap,
}

/// A hole the producer states, and why it is there.
///
/// An excluded path is not a gap. The reason is required because a hole with no
/// reason cannot be told apart from an oversight, and telling a reader *why*
/// the material is absent is the point of declaring it at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusion {
    /// The structural path left out, as a prefix.
    pub path: String,
    /// Why it is absent, in the producer's words.
    pub reason: String,
}

/// The release points a dataset means to carry.
///
/// Reported rather than checked: a declared range that is not held is not
/// counted as a gap, because a date owns nothing and a work need not exist on
/// every date in a range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

/// What a producer says a dataset is meant to carry.
///
/// Absent means nothing was declared, and every question about the scope is
/// answered from the contents alone, exactly as before declarations existed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    /// Structural path prefixes this dataset means to hold.
    pub intends: Vec<String>,
    /// Holes inside `intends`, each with its reason.
    pub excludes: Vec<Exclusion>,
    /// The release points intended, when the producer wants to state them.
    pub dates: Option<DateRange>,
    /// The link namespaces a reader should expect to meet
    /// (`docs/adr/0002-links-live-in-the-core.md`).
    pub namespaces: Vec<String>,
}

impl Declaration {
    /// Whether this dataset says it carries links in a namespace.
    pub fn declares_namespace(&self, namespace: &str) -> bool {
        self.namespaces.iter().any(|declared| declared == namespace)
    }

    /// Whether a path sits inside a stated hole.
    fn is_excluded(&self, path: &str) -> bool {
        self.excludes
            .iter()
            .any(|hole| covers_path(&hole.path, path))
    }

    /// Whether a path sits inside what this declaration intends.
    fn intends_path(&self, path: &str) -> bool {
        self.intends.iter().any(|want| covers_path(want, path))
    }
}

/// Whether `prefix` names `path` or an ancestor of it.
///
/// Compares whole segments: `uscode/title_2` must not answer for
/// `uscode/title_26`.
fn covers_path(prefix: &str, path: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// One work a dataset holds, and when it was published.
///
/// The work and its dates are kept together rather than as two flat lists. A
/// dataset holding title 9 only in July and title 51 only in August holds no
/// title 9 in August, and a pair of parallel lists cannot say so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCoverage {
    pub work: WorkId,
    /// The dates this work was published, oldest first.
    pub dates: Vec<String>,
}

/// What a dataset covers: what it holds, and what it said it would.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scope {
    /// Every work held, with its dates. In work order.
    pub held: Vec<WorkCoverage>,
    /// What the producer said this dataset would carry, when they said anything.
    pub declared: Option<Declaration>,
}

impl Scope {
    /// Derive the scope of anything that can read documents.
    ///
    /// This reads keys only. Asking what a dataset holds no longer costs the
    /// text of everything it holds.
    pub fn derive<R: DocumentReader + ?Sized>(reader: &R) -> Result<Self, DatasetError> {
        let mut held = Vec::new();
        for work in reader.works()? {
            let dates = reader
                .expressions(&work)?
                .into_iter()
                .map(|info| info.id.at)
                .collect();
            held.push(WorkCoverage { work, dates });
        }
        Ok(Self {
            held,
            declared: reader.metadata().declaration.clone(),
        })
    }

    /// Declared material that is not excluded and is not held, in declared order.
    ///
    /// Every entry is an *unexplained* absence, so a non-empty result means one
    /// thing: the build did not do what it said it would. A hole the producer
    /// stated is an [`Exclusion`] and is deliberately not reported here.
    pub fn gaps(&self) -> Vec<String> {
        let Some(declared) = &self.declared else {
            return Vec::new();
        };
        declared
            .intends
            .iter()
            .filter(|want| !declared.is_excluded(want) && !self.holds(want))
            .cloned()
            .collect()
    }

    /// Whether any held work sits at this path or beneath it.
    fn holds(&self, path: &str) -> bool {
        self.works().any(|work| covers_path(path, work.as_str()))
    }

    /// Every work held, in work order.
    pub fn works(&self) -> impl Iterator<Item = &WorkId> {
        self.held.iter().map(|coverage| &coverage.work)
    }

    /// Every date any work was published, sorted and deduplicated.
    ///
    /// A convenience for display. It says nothing about which work existed on
    /// which date; ask [`Scope::held`] for that.
    ///
    /// [`Scope::held`]: Scope#structfield.held
    pub fn dates(&self) -> Vec<String> {
        let mut dates: Vec<String> = self
            .held
            .iter()
            .flat_map(|coverage| coverage.dates.iter().cloned())
            .collect();
        dates.sort();
        dates.dedup();
        dates
    }

    /// Whether this dataset covers the material a path names.
    ///
    /// A path is in scope when it sits inside a held work, or when it names an
    /// ancestor of one: asking about `uscode` is in scope for a dataset holding
    /// `uscode/title_9`, because the dataset does hold part of it.
    ///
    /// Held material is always `InScope`, even when the declaration never
    /// mentioned it. A producer who under-declares is careless, and answering
    /// `OutOfScope` for material we demonstrably hold would be a false
    /// statement about our own contents.
    ///
    /// A dataset that declared nothing answers exactly as it did before
    /// declarations existed: held or not held, and no third answer.
    pub fn covers(&self, path: &str) -> Coverage {
        let inside_held = self
            .works()
            .any(|work| path == work.as_str() || path.starts_with(&format!("{work}/")));
        let ancestor_of_held = self
            .works()
            .any(|work| work.as_str().starts_with(&format!("{path}/")));

        if inside_held || ancestor_of_held {
            return Coverage::InScope;
        }

        // Not held. Whether that is a gap or simply out of the lane is the
        // question only a declaration can answer.
        match &self.declared {
            Some(declared) if declared.intends_path(path) && !declared.is_excluded(path) => {
                Coverage::Gap
            }
            _ => Coverage::OutOfScope,
        }
    }
}
