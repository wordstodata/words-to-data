//! A method: what made a statement, and which version of it.
//!
//! A statement already says who made it and how. What it could not say is
//! whether that *how* is still the current *how*. A rule is edited, a prompt is
//! rewritten, a cutoff moves, and the name stays the same while the answers
//! change underneath. A reader then cannot tell a statement this build would
//! make again from one it would not.
//!
//! So a method has an identity of a name and a version, and the version is
//! **chosen by a person**, when the method's answers change (#179, decision
//! 10). A hash of the method's parameters was considered and rejected: it
//! churns on cosmetic edits, and a number that moves for no reason is a number
//! everybody learns to ignore. Do not reintroduce one.
//!
//! The version is a whole number rather than free text for the same reason. A
//! string invites a date, a git revision, or the parameter hash this decision
//! refuses.

use serde::{Deserialize, Serialize};

/// What made a statement, at the version it was at when it made it.
///
/// A **record**: it says what the maker asserted about their own work, and it
/// is not recomputable from anything else in the dataset
/// (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Method {
    /// The reasoning, named. An open string, like a link's kind: another party
    /// names their own method without our permission.
    pub name: String,
    /// Which version of that reasoning. Raised by a person when the method's
    /// answers change, and never by a build.
    pub version: u32,
}

impl Method {
    pub fn new(name: impl Into<String>, version: u32) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }
}

impl std::fmt::Display for Method {
    /// `name@version`, which is how a report names a method in one column.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}
