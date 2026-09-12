//! Works and expressions: naming a document without a date, and with one.
//!
//! A **work** is a legal document as a concept, with no date attached. Title 9
//! is one work, from 1947 until today. An **expression** is that work as it read
//! on one date.
//!
//! The distinction matters because storage used to key on a global release
//! date: `get_version("2025-07-18")` asked for the whole tree at a moment. That
//! fits the US Code, where every title is republished together. It fits nothing
//! else. A court opinion is a work with a single expression, and it belongs to
//! no release cycle at all, so a dataset holding opinions has no global date to
//! key on.
//!
//! These are now the storage keys themselves, not a view over a global version
//! list (`docs/adr/0003-storage-is-keyed-by-work.md`).

use serde::{Deserialize, Serialize};

use crate::document::DocumentNode;

/// A legal document as a concept, with no date attached.
///
/// Named by its structural path, such as `uscode/title_9`. That path locates
/// rather than identifies, so this becomes a stable identity when one exists
/// (`docs/adr/0001-structural-paths-locate-not-identify.md`). The path is held
/// privately so that change costs no call site.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkId(String);

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

/// Text that does not name a work and a date.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{0}` does not name an expression. Write it as `<work>@<date>`, such as \
     `uscode/title_9@2025-07-18`."
)]
pub struct ParseExpressionIdError(pub String);

impl std::str::FromStr for ExpressionId {
    type Err = ParseExpressionIdError;

    /// Read `uscode/title_9@2025-07-18` back into an identifier.
    ///
    /// A work is a structural path and holds slashes; a date holds dashes.
    /// Neither holds an `@`, so the split point is unambiguous.
    ///
    /// The date is checked rather than taken on trust. A typo that survives as
    /// text becomes a lookup that finds nothing, and an empty answer that means
    /// "you mistyped it" is the failure this project exists to prevent.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let refuse = || ParseExpressionIdError(text.to_string());

        let (work, at) = text.split_once('@').ok_or_else(refuse)?;
        if work.is_empty() {
            return Err(refuse());
        }
        crate::date::date_str_to_date(at).map_err(|_| refuse())?;

        Ok(Self::new(WorkId::new(work), at))
    }
}

/// One work as it read on one date, with its text.
///
/// This is the unit a dataset stores. Nothing above it groups expressions by
/// date, so two documents published on unrelated days sit side by side without
/// either pretending to share a release cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expression {
    pub id: ExpressionId,
    /// Optional human-readable name for this printing, such as
    /// `Pre-Tax Cuts Act`. Names one expression, not a moment across the
    /// dataset, so two works may carry the same label.
    pub label: Option<String>,
    /// The root of the document tree as it read on that date.
    ///
    /// Named `root` rather than `element`: this is the top of a tree of
    /// [`DocumentNode`]s, and "element" is USLM's word for a node, which the
    /// core no longer speaks (#129).
    pub root: DocumentNode,
}

/// One expression's headline facts, without its tree.
///
/// Listing what a dataset holds must not cost the text of everything it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionInfo {
    pub id: ExpressionId,
    pub label: Option<String>,
}

/// Which works a job spanning two dates can and cannot cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorksBetween {
    /// Works held on both dates, in work order, as a pair to diff.
    pub pairs: Vec<crate::dataset::ExpressionPair>,
    /// Works held on one of the dates but not the other.
    pub skipped: Vec<WorkId>,
}

/// Every work this dataset holds on both dates, paired for diffing.
///
/// A diff is between two expressions of one work, so a job that spans a corpus
/// is one diff per document rather than one over everything. This decides which
/// documents that is, once, for every command that needs it.
///
/// A work published on only one of the two dates cannot be diffed between them.
/// It is returned in `skipped` rather than dropped: a run that quietly covered
/// forty of fifty-eight works would report success for a job it did not do.
pub fn works_between<R: crate::storage::DocumentReader + ?Sized>(
    reader: &R,
    from: &str,
    to: &str,
) -> Result<WorksBetween, crate::dataset::DatasetError> {
    let mut pairs = Vec::new();
    let mut skipped = Vec::new();

    for work in reader.works()? {
        let dates: Vec<String> = reader
            .expressions(&work)?
            .into_iter()
            .map(|info| info.id.at)
            .collect();

        if dates.iter().any(|d| d == from) && dates.iter().any(|d| d == to) {
            pairs.push((
                ExpressionId::new(work.clone(), from),
                ExpressionId::new(work, to),
            ));
        } else {
            skipped.push(work);
        }
    }

    Ok(WorksBetween { pairs, skipped })
}

/// Split a parsed tree into the works it holds.
///
/// A root that names a work, such as `uscode/title_9`, is one work and is
/// returned whole. A bare container root, such as the `uscode` a merged release
/// point produces, holds one work per child and is taken apart.
///
/// The container is where a release cycle used to live. Dropping it is what
/// lets a document that belongs to no release cycle enter a dataset at all.
pub fn work_roots(root: DocumentNode) -> Vec<DocumentNode> {
    if root.data.path.contains('/') {
        return vec![root];
    }
    root.children
}
