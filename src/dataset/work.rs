//! Works and expressions: naming a document without a date, and with one.
//!
//! A **work** is a legal document as a concept, with no date attached. Title 9
//! is one work, from 1947 until today. An **expression** is that work as it read
//! on one date.
//!
//! The distinction matters because the current interface keys on a global
//! release date: `get_version("2025-07-18")` asks for the whole tree at a
//! moment. That fits the US Code, where every title is republished together. It
//! fits nothing else. A court opinion is a work with a single expression, and it
//! belongs to no release cycle at all, so a dataset holding opinions has no
//! global date to key on.
//!
//! This module adds the work-scoped view alongside the date-keyed one. It does
//! not remove the older shape yet: the global version list still exists, and
//! expressions are still discovered through it.

use serde::{Deserialize, Serialize};

/// A legal document as a concept, with no date attached.
///
/// Named by its structural path, such as `uscode/title_9`. That path locates
/// rather than identifies, so this becomes a stable identity when one exists
/// (`docs/adr/0001-structural-paths-locate-not-identify.md`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkId(pub String);

impl WorkId {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One work as it read on one date.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExpressionId {
    /// The work this is an expression of.
    pub work: WorkId,
    /// The date it read this way, `YYYY-MM-DD`.
    ///
    /// For a statute this is the release point. For a document published once
    /// and never amended, such as a court opinion, it is the date of
    /// publication, and there will only ever be one.
    pub at: String,
}

impl ExpressionId {
    pub fn new(work: WorkId, at: impl Into<String>) -> Self {
        Self {
            work,
            at: at.into(),
        }
    }
}

impl std::fmt::Display for ExpressionId {
    /// `uscode/title_9@2025-07-18`, the form Akoma Ntoso and ELI use.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.work, self.at)
    }
}
