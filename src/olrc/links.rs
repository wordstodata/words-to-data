//! A classification row, as the links it states.

use std::collections::BTreeSet;

use crate::citation::resolve::SectionPaths;
use crate::dataset::{Coverage, Scope};
use crate::link::{KindPayload, Link, LinkKind, Provenance, Target, VerificationState};

use super::ClassificationRow;

/// The namespace a classification's payload is written in.
pub const NAMESPACE: &str = "olrc";

/// How one section of a public law is named as a link object:
/// `olrc.classification:119-21:71301(a)`.
///
/// The public law and the section of it, so a reader that does not know this
/// namespace can still say what the link points at.
pub fn classification_reference(public_law: &str, law_section: &str) -> String {
    format!("olrc.classification:{public_law}:{law_section}")
}

/// What a set of rows came to: the links they state, and each row that states
/// none, with the reason.
#[derive(Debug, Clone, Default)]
pub struct Classified {
    pub links: Vec<Link>,
    pub skipped: Vec<Skipped>,
}

/// A row that states no link, and why.
#[derive(Debug, Clone)]
pub struct Skipped {
    pub row: ClassificationRow,
    pub reason: SkipReason,
}

/// Why a row states no link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The dataset does not carry the title, so it can say nothing about the
    /// section. Out of scope, and not "not found".
    TitleNotHeld,
    /// The dataset said it would carry the title and does not: a fault in the
    /// build rather than a statement about the law.
    TitleMissing,
    /// The dataset holds the title and no section with this number, at any date
    /// it holds.
    SectionNotHeld,
}

/// The links a set of rows states, against what a dataset holds.
///
/// A row becomes one link for each section of the law it names and each path
/// its Code section sits at. `source` names the table the rows were read from.
pub fn classify(
    rows: &[ClassificationRow],
    scope: &Scope,
    paths: &SectionPaths,
    source: &str,
) -> Classified {
    let mut classified = Classified::default();

    for row in rows {
        let skip = |reason| Skipped {
            row: row.clone(),
            reason,
        };
        match scope.covers(&format!("uscode/title_{}", row.title)) {
            Coverage::OutOfScope => {
                classified.skipped.push(skip(SkipReason::TitleNotHeld));
                continue;
            }
            Coverage::Gap => {
                classified.skipped.push(skip(SkipReason::TitleMissing));
                continue;
            }
            Coverage::InScope => {}
        }

        let uslm_id = format!("/us/usc/t{}/s{}", row.title, row.section);
        let held: BTreeSet<&String> = paths.paths_of(&uslm_id).iter().collect();
        if held.is_empty() {
            classified.skipped.push(skip(SkipReason::SectionNotHeld));
            continue;
        }

        for path in held {
            for law_section in row.named_law_sections() {
                classified.links.push(link(row, path, &law_section, source));
            }
        }
    }

    classified
}

/// One link: the Code section at `path`, classified from `law_section`.
fn link(row: &ClassificationRow, path: &str, law_section: &str, source: &str) -> Link {
    Link {
        subject: Target::Node(path.to_string()),
        kind: LinkKind::new(LinkKind::CLASSIFIED_FROM),
        object: Target::External {
            reference: classification_reference(&row.public_law, law_section),
            display: format!("Pub. L. {}, § {law_section}", row.public_law),
        },
        provenance: Provenance {
            source: source.to_string(),
            method: None,
            verification: VerificationState::Asserted,
            evidence: None,
            raw_score: None,
            timestamp: None,
            corroboration: None,
        },
        payload: Some(KindPayload {
            namespace: NAMESPACE.to_string(),
            value: serde_json::json!({ "description": row.description }),
        }),
    }
}
