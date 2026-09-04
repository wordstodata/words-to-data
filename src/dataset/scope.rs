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
//! Today the scope is **derived** from the contents. A producer cannot yet
//! declare an intent, so "title 26 minus section 174, because the source
//! failed" is not sayable, and neither is the gap between what a dataset meant
//! to carry and what it does. That needs a declaration on the dataset, and it
//! is the next step.

use serde::{Deserialize, Serialize};

use crate::uslm::USLMElement;

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

/// What a dataset covers, derived from its contents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scope {
    /// The coverage units held, such as `uscode/title_9`. Sorted, deduplicated.
    pub held: Vec<String>,
    /// The dates held, sorted.
    pub dates: Vec<String>,
}

impl Scope {
    /// Build a scope from the root element of each version.
    pub fn from_versions<'a>(versions: impl Iterator<Item = (&'a str, &'a USLMElement)>) -> Self {
        let mut held = Vec::new();
        let mut dates = Vec::new();

        for (date, root) in versions {
            dates.push(date.to_string());
            held.extend(coverage_units(root));
        }

        held.sort();
        held.dedup();
        dates.sort();
        dates.dedup();
        Self { held, dates }
    }

    /// Whether this dataset covers the material a path names.
    ///
    /// A path is in scope when it sits inside a held unit, or when it names an
    /// ancestor of one: asking about `uscode` is in scope for a dataset holding
    /// `uscode/title_9`, because the dataset does hold part of it.
    pub fn covers(&self, path: &str) -> Coverage {
        let inside_held = self
            .held
            .iter()
            .any(|unit| path == unit || path.starts_with(&format!("{unit}/")));
        let ancestor_of_held = self
            .held
            .iter()
            .any(|unit| unit.starts_with(&format!("{path}/")));

        if inside_held || ancestor_of_held {
            Coverage::InScope
        } else {
            Coverage::OutOfScope
        }
    }
}

/// The coverage unit or units one version's root element stands for.
///
/// A root that already names a unit, such as `uscode/title_9`, is the unit. A
/// bare container root, such as `uscode`, stands for each of its children.
fn coverage_units(root: &USLMElement) -> Vec<String> {
    let path = root.data.path.to_string();
    if path.contains('/') {
        return vec![path];
    }
    root.children
        .iter()
        .map(|child| child.data.path.to_string())
        .collect()
}
