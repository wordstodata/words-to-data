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
//! Today the scope is **derived** from the contents. A producer cannot yet
//! declare an intent, so "title 26 minus section 174, because the source
//! failed" is not sayable, and neither is the gap between what a dataset meant
//! to carry and what it does. That needs a declaration on the dataset, and it
//! is the next step.

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

/// What a dataset covers, derived from its contents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scope {
    /// Every work held, with its dates. In work order.
    pub held: Vec<WorkCoverage>,
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
        Ok(Self { held })
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
    pub fn covers(&self, path: &str) -> Coverage {
        let inside_held = self
            .works()
            .any(|work| path == work.as_str() || path.starts_with(&format!("{work}/")));
        let ancestor_of_held = self
            .works()
            .any(|work| work.as_str().starts_with(&format!("{path}/")));

        if inside_held || ancestor_of_held {
            Coverage::InScope
        } else {
            Coverage::OutOfScope
        }
    }
}
