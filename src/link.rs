//! Links: the statements that connect a provision to something else.
//!
//! A link is core, not part of any extension, so a reader that does not know
//! an extension can still see a link, report it, and preserve it when it writes
//! the file again. Silent loss is the one failure a portable format cannot have
//! (`docs/adr/0002-links-live-in-the-core.md`).
//!
//! The kind is a namespaced string rather than an enum we control, so another
//! party can add a link type without our permission. `legislature.amended_by`
//! is ours. `judicial.cites` is ours. `westlaw.headnote` would not be.
//!
//! This module is additive today: [`ChangeAnnotation`] remains the stored form,
//! and [`Link::from_annotation`] projects it into this shape. The storage
//! migration is separate work.

use serde::{Deserialize, Serialize};

use crate::annotation::{AnnotationStatus, ChangeAnnotation};

/// The kind of a link, namespaced by the extension that defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkKind(pub String);

impl LinkKind {
    /// A change to the law, caused by an amendment in a bill.
    pub const AMENDED_BY: &'static str = "legislature.amended_by";

    /// An opinion citing a provision.
    pub const CITES: &'static str = "judicial.cites";

    pub fn new(kind: impl Into<String>) -> Self {
        Self(kind.into())
    }

    /// The namespace half, `legislature` in `legislature.amended_by`.
    ///
    /// A reader uses this to decide whether it understands a link. It may not,
    /// and that is allowed: it still carries the link.
    pub fn namespace(&self) -> &str {
        self.0.split_once('.').map_or(&self.0, |(before, _)| before)
    }
}

/// What a link points at.
///
/// A closed set on purpose. A reader that does not know the legislature
/// extension can still report "this change was caused by something, and here is
/// its name", and can still tell a reference inside the dataset from one that
/// leaves it, because only the first can be checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// A provision in this dataset, by structural path.
    ///
    /// The path locates rather than identifies, so this moves to a stable
    /// provision identity when one exists
    /// (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    Provision(String),
    /// A provision as it read on one date.
    Expression { path: String, date: String },
    /// Something outside the core model, named in an extension's namespace.
    /// An amendment is reached this way, because amendments are legislature.
    External { reference: String, display: String },
}

/// How much trust a statement has earned.
///
/// Not a confidence number. A raw model score is not calibrated, so publishing
/// one as a probability claims a precision we do not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    /// A source says so.
    Asserted,
    /// A machine proposed it, and no human has confirmed it.
    MachineSuggested,
    /// A human confirmed it.
    HumanConfirmed,
    /// Contested, or found to be wrong.
    Disputed,
}

/// Where a statement came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Who or what made the statement: `model:local`, `human:jesse`.
    pub source: String,
    /// How it was made, when that is known and worth recording.
    pub method: Option<String>,
    /// How far the statement can be trusted.
    pub verification: VerificationState,
    /// What the statement was based on.
    ///
    /// `None` for everything produced so far: raw model replies were never
    /// persisted (#58), so the evidence behind those claims is gone.
    pub evidence: Option<String>,
    /// The raw score a model reported, kept as diagnostic data only. It is not
    /// a probability and must not be presented as one.
    pub raw_score: Option<f32>,
}

/// A statement connecting a provision to something else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub subject: Target,
    pub kind: LinkKind,
    pub object: Target,
    pub provenance: Provenance,
}

impl Link {
    /// Project a stored annotation into links, one per path it covers.
    ///
    /// An annotation names several paths when one amendment changed several
    /// provisions. Each is its own statement, because each can be confirmed or
    /// disputed on its own.
    pub fn from_annotation(annotation: &ChangeAnnotation) -> Vec<Link> {
        let provenance = Provenance {
            source: annotation.metadata.annotator.clone(),
            method: Some(format!("{:?}", annotation.operation)),
            verification: verification_of(annotation),
            evidence: None,
            raw_score: annotation.metadata.confidence,
        };

        annotation
            .paths
            .iter()
            .map(|path| Link {
                subject: Target::Provision(path.clone()),
                kind: LinkKind::new(LinkKind::AMENDED_BY),
                object: Target::External {
                    reference: format!(
                        "legislature.amendment:{}",
                        annotation.source_bill.amendment_id
                    ),
                    display: annotation.source_bill.causative_text.clone(),
                },
                provenance: provenance.clone(),
            })
            .collect()
    }
}

/// Map a stored status onto a verification state.
///
/// `Pending` is not a trust level, it is the absence of review, so the maker
/// decides: a machine's unreviewed claim is `MachineSuggested`, a person's is
/// `Asserted`.
///
/// `Rejected` collapses into `Disputed`, which loses information: "found to be
/// wrong" is a stronger statement than "contested". The four states in
/// `CONTEXT.md` have no home for a refuted claim, and that gap is real.
fn verification_of(annotation: &ChangeAnnotation) -> VerificationState {
    match annotation.metadata.status {
        AnnotationStatus::Verified => VerificationState::HumanConfirmed,
        AnnotationStatus::Disputed | AnnotationStatus::Rejected => VerificationState::Disputed,
        AnnotationStatus::Pending => {
            if annotation.metadata.annotator.starts_with("model:") {
                VerificationState::MachineSuggested
            } else {
                VerificationState::Asserted
            }
        }
    }
}
