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

use crate::dataset::WorkId;

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

/// One method, at one version, applied to one window.
///
/// A dataset used to keep no list of what had been done to it, so a missing
/// step read as a complete file and an agent could not decide what to do next
/// (#179, decision 11).
///
/// It records which **method** ran, not which step. "`redesignations` has run
/// here" stays true for ever while the thing it means changes underneath. "This
/// reasoning was applied to this window" is what an agent can act on.
///
/// A **record**, and it passes the test that keeps this honest: "method M at
/// version V ran over window W" is something that happened, and it stays true
/// however much the dataset grows. A question whose answer changes as the
/// dataset grows — how many statements are unplaced, for one — is derived and
/// must not be stored here (#153,
/// `docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
///
/// It carries no clock reading, and none is wanted. A re-run of the same method
/// at the same version over the same window is the same record, which is what
/// makes a rebuild idempotent; a timestamp would turn each re-run into a new
/// row saying the same thing.
///
/// The window is one work and two dates rather than two [`crate::dataset::ExpressionId`]s,
/// for the reason `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`
/// gives for `Target::Change`: two copies of one work can disagree, and after an
/// edit one of them will.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MethodRun {
    /// What ran, and which version of it.
    pub method: Method,
    /// The work whose two expressions make the window.
    pub work: WorkId,
    /// The earlier expression's date.
    pub from_date: String,
    /// The later expression's date.
    pub to_date: String,
}

impl MethodRun {
    /// Whether this run covers the window between two dates of one work.
    pub fn covers(&self, work: &WorkId, from_date: &str, to_date: &str) -> bool {
        &self.work == work && self.from_date == from_date && self.to_date == to_date
    }
}
